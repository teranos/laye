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
    access_key_id             = aws_iam_access_key.relaye.id
    secret_access_key         = aws_iam_access_key.relaye.secret
    aws_region                = var.aws_region
    artifacts_bucket          = aws_s3_bucket.relaye_artifacts.id
    relaye_topics             = var.relaye_topics
    identity_parameter_name   = aws_ssm_parameter.relaye_identity.name
    atproto_key_parameter_name = aws_ssm_parameter.relaye_atproto_client_key.name
  })

  # The instance's userdata fetches the identity from SSM Parameter
  # Store before starting relaye. Make sure the parameters + the IAM
  # policy statement are in place before Lightsail boots the box.
  depends_on = [
    aws_ssm_parameter.relaye_identity,
    aws_ssm_parameter.relaye_atproto_client_key,
    aws_iam_user_policy.relaye_box,
  ]
}

resource "aws_lightsail_instance_public_ports" "relaye" {
  instance_name = aws_lightsail_instance.relaye.name

  port_info {
    from_port  = 22
    to_port    = 22
    protocol   = "tcp"
    cidrs      = ["0.0.0.0/0"]
    ipv6_cidrs = ["::/0"]
  }

  port_info {
    from_port  = var.relaye_origin_port
    to_port    = var.relaye_origin_port
    protocol   = "tcp"
    cidrs      = ["0.0.0.0/0"]
    ipv6_cidrs = ["::/0"]
  }
}
