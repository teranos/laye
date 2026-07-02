variable "aws_region" {
  type    = string
  default = "eu-central-1"
}

variable "aws_profile" {
  type    = string
  default = "sbvh"
}

variable "root_domain" {
  type    = string
  default = "sbvh.nl"
}

variable "relaye_subdomain" {
  type    = string
  default = "relaye"
}

variable "relaye_origin_port" {
  type    = number
  default = 9001
}

variable "relaye_origin_domain" {
  type    = string
  default = "origin-relaye.sbvh.nl"
}

variable "relaye_topics" {
  type    = string
  default = "rave-positions/v1,rave-chat/v1"
}

variable "relaye_artifacts_bucket" {
  type    = string
  default = "laye-relaye-artifacts"
}

# Base64 of the raw Ed25519 keypair backing relaye's libp2p PeerId.
# Persisted in AWS Secrets Manager so the PeerId survives instance
# replacement — rave hardcodes the dial multiaddr with this PeerId
# (see `rave/src/lib.rs:55` in tsot-roam); regenerating identity per
# box breaks every client. Seeded once out-of-band, e.g.
#   TF_VAR_relaye_identity_bytes_b64="$(ssh box 'sudo cat /var/lib/relaye/identity.bin | base64')"
# at apply time. Kept out of source; never checked in.
variable "relaye_identity_bytes_b64" {
  type      = string
  sensitive = true
}

variable "github_repo" {
  type    = string
  default = "teranos/laye"
}

variable "bevy_starter_subdomain" {
  type    = string
  default = "bevy-starter"
}

variable "bevy_starter_bucket" {
  type    = string
  default = "laye-bevy-starter-static"
}
