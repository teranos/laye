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

resource "aws_s3_bucket" "bevy_starter_static" {
  bucket = var.bevy_starter_bucket
}

resource "aws_s3_bucket_public_access_block" "bevy_starter_static" {
  bucket                  = aws_s3_bucket.bevy_starter_static.id
  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_policy" "bevy_starter_static" {
  bucket = aws_s3_bucket.bevy_starter_static.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Sid    = "AllowCloudFrontServicePrincipalRead"
      Effect = "Allow"
      Principal = {
        Service = "cloudfront.amazonaws.com"
      }
      Action   = ["s3:GetObject"]
      Resource = "${aws_s3_bucket.bevy_starter_static.arn}/*"
      Condition = {
        StringEquals = {
          "AWS:SourceArn" = aws_cloudfront_distribution.bevy_starter.arn
        }
      }
    }]
  })
}
