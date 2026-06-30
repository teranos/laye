locals {
  relaye_fqdn        = "${var.relaye_subdomain}.${var.root_domain}"
  bevy_starter_fqdn  = "${var.bevy_starter_subdomain}.${var.root_domain}"
  github_repo_parts  = split("/", var.github_repo)
  github_owner       = local.github_repo_parts[0]
  github_repo_name   = local.github_repo_parts[1]
}
