#!/bin/bash
set -eu

# 2 GiB swap before anything else. Lightsail nano has 414 MiB RAM
# and zero swap by default; under memory pressure the kernel OOM
# killer takes systemd services and the network-config layer with
# it (verified: 2026-07-01 06:43:16 UTC — ens5 route install timed
# out and systemd-journald crashed, box became externally
# unreachable for ~18 h until reboot). Swap absorbs transient
# spikes from unattended-upgrades/fwupd/snapd and prevents the
# same cascade recurring on any freshly-provisioned box.
if ! swapon --show | grep -q '/swapfile'; then
  fallocate -l 2G /swapfile
  chmod 600 /swapfile
  mkswap /swapfile
  swapon /swapfile
  grep -q '^/swapfile' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
fi

apt-get update
apt-get install -y curl unzip
curl -s 'https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip' -o /tmp/awscliv2.zip
unzip -q /tmp/awscliv2.zip -d /tmp/
/tmp/aws/install
rm -rf /tmp/aws /tmp/awscliv2.zip

mkdir -p /root/.aws
cat > /root/.aws/credentials <<EOF
[default]
aws_access_key_id = ${access_key_id}
aws_secret_access_key = ${secret_access_key}
region = ${aws_region}
EOF
chmod 600 /root/.aws/credentials

mkdir -p /var/lib/relaye

# Fetch the stable relaye identity from Secrets Manager so this box's
# libp2p PeerId matches the one rave hardcodes in its dial multiaddr.
# Retries a few times because IAM policy propagation from the same
# tofu run can lag first-boot by seconds. `set -eu` at the top of
# this script means a persistently-failing fetch aborts provisioning
# — that's intentional: silent fresh-mint would produce a new PeerId
# and break every client.
for attempt in 1 2 3 4 5 6; do
  if aws secretsmanager get-secret-value \
       --secret-id "${identity_secret_id}" \
       --region "${aws_region}" \
       --query SecretString \
       --output text \
       | base64 -d > /var/lib/relaye/identity.bin; then
    break
  fi
  echo "identity fetch attempt $attempt failed; sleeping 5s" >&2
  sleep 5
done
chmod 600 /var/lib/relaye/identity.bin
test -s /var/lib/relaye/identity.bin  # non-empty; abort if not

cat > /etc/systemd/system/relaye.service <<'UNIT'
[Unit]
Description=laye libp2p relay
After=network.target

[Service]
ExecStart=/usr/local/bin/relaye
Restart=always
RestartSec=5
Environment=RELAYE_IDENTITY_FILE=/var/lib/relaye/identity.bin
Environment=RELAYE_TOPICS=${relaye_topics}
MemoryMax=400M

[Install]
WantedBy=multi-user.target
UNIT

cat > /usr/local/bin/relaye-update <<UPDATE
#!/bin/bash
set -eu
NEW=\$(mktemp)
aws s3 cp "s3://${artifacts_bucket}/relaye" "\$NEW" 2>/dev/null || { rm -f "\$NEW"; exit 0; }
chmod +x "\$NEW"
if ! cmp -s "\$NEW" /usr/local/bin/relaye 2>/dev/null; then
  mv "\$NEW" /usr/local/bin/relaye
  systemctl restart relaye
else
  rm "\$NEW"
fi
UPDATE
chmod +x /usr/local/bin/relaye-update

cat > /etc/systemd/system/relaye-update.service <<'UPSVC'
[Unit]
Description=Pull latest relaye binary from S3

[Service]
Type=oneshot
ExecStart=/usr/local/bin/relaye-update
UPSVC

cat > /etc/systemd/system/relaye-update.timer <<'UPTMR'
[Unit]
Description=Run relaye-update every 30s

[Timer]
OnBootSec=10
OnUnitActiveSec=30s

[Install]
WantedBy=timers.target
UPTMR

systemctl daemon-reload
systemctl enable relaye.service
systemctl enable --now relaye-update.timer
