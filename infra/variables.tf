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

variable "github_repo" {
  type    = string
  default = "teranos/laye"
}
