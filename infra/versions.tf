terraform {
  required_version = "= 1.11.6"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "= 6.50.0"
    }
    github = {
      source  = "integrations/github"
      version = "= 6.6.0"
    }
  }

  backend "s3" {
    bucket  = "tfstate.sbvh"
    key     = "laye"
    region  = "eu-central-1"
    profile = "sbvh"
    encrypt = true
  }
}
