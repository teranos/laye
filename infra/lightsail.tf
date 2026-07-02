resource "aws_lightsail_key_pair" "relaye" {
  name = "relaye"
}

resource "aws_lightsail_instance" "relaye" {
  name              = "relaye-eu-1"
  availability_zone = "eu-central-1a"
  blueprint_id      = "ubuntu_24_04"
  bundle_id         = "nano_3_0"
  key_pair_name     = aws_lightsail_key_pair.relaye.name

  user_data = templatefile("${path.module}/userdata/relaye.sh", {
    access_key_id      = aws_iam_access_key.relaye.id
    secret_access_key  = aws_iam_access_key.relaye.secret
    aws_region         = var.aws_region
    artifacts_bucket   = aws_s3_bucket.relaye_artifacts.id
    relaye_topics      = var.relaye_topics
    identity_secret_id = aws_secretsmanager_secret.relaye_identity.id
  })

  # The instance's userdata fetches the identity from Secrets Manager
  # before starting relaye. Make sure the secret version + the IAM
  # policy statement are in place before Lightsail boots the box.
  depends_on = [
    aws_secretsmanager_secret_version.relaye_identity,
    aws_iam_user_policy.relaye_box,
  ]
}

resource "aws_lightsail_instance_public_ports" "relaye" {
  instance_name = aws_lightsail_instance.relaye.name

  port_info {
    from_port = 22
    to_port   = 22
    protocol  = "tcp"
  }

  port_info {
    from_port = var.relaye_origin_port
    to_port   = var.relaye_origin_port
    protocol  = "tcp"
  }
}
