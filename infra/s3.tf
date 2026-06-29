resource "aws_s3_bucket" "relaye_artifacts" {
  bucket = var.relaye_artifacts_bucket
}

resource "aws_s3_bucket_public_access_block" "relaye_artifacts" {
  bucket                  = aws_s3_bucket.relaye_artifacts.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_versioning" "relaye_artifacts" {
  bucket = aws_s3_bucket.relaye_artifacts.id
  versioning_configuration {
    status = "Enabled"
  }
}
