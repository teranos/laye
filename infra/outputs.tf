output "relaye_fqdn" {
  value = local.relaye_fqdn
}

output "relaye_public_ip" {
  value = aws_lightsail_instance.relaye.public_ip_address
}

output "relaye_distribution_id" {
  value = aws_cloudfront_distribution.relaye.id
}

output "relaye_artifacts_bucket" {
  value = aws_s3_bucket.relaye_artifacts.id
}

output "relaye_private_key" {
  value     = aws_lightsail_key_pair.relaye.private_key
  sensitive = true
}

output "github_deploy_role_arn" {
  value = aws_iam_role.github_deploy.arn
}

output "bevy_starter_fqdn" {
  value = local.bevy_starter_fqdn
}

output "bevy_starter_bucket" {
  value = aws_s3_bucket.bevy_starter_static.id
}

output "bevy_starter_distribution_id" {
  value = aws_cloudfront_distribution.bevy_starter.id
}
