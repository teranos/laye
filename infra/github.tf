resource "github_actions_variable" "bevy_starter_distribution_id" {
  repository    = local.github_repo_name
  variable_name = "BEVY_STARTER_DISTRIBUTION_ID"
  value         = aws_cloudfront_distribution.bevy_starter.id
}
