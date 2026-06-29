#!/bin/bash
set -eu

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
