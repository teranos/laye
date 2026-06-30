data "aws_cloudfront_cache_policy" "caching_disabled" {
  name = "Managed-CachingDisabled"
}

data "aws_cloudfront_cache_policy" "caching_optimized" {
  name = "Managed-CachingOptimized"
}

data "aws_cloudfront_origin_request_policy" "all_viewer" {
  name = "Managed-AllViewer"
}

resource "aws_cloudfront_distribution" "relaye" {
  enabled     = true
  comment     = "laye libp2p relay (WebSocket)"
  price_class = "PriceClass_100"

  aliases = [local.relaye_fqdn]

  origin {
    domain_name = var.relaye_origin_domain
    origin_id   = "lightsail-relaye"

    custom_origin_config {
      http_port                = var.relaye_origin_port
      https_port               = 443
      origin_protocol_policy   = "http-only"
      origin_ssl_protocols     = ["TLSv1.2"]
      origin_read_timeout      = 60
      origin_keepalive_timeout = 60
    }
  }

  default_cache_behavior {
    target_origin_id       = "lightsail-relaye"
    viewer_protocol_policy = "https-only"

    allowed_methods = ["GET", "HEAD", "OPTIONS", "PUT", "POST", "PATCH", "DELETE"]
    cached_methods  = ["GET", "HEAD"]

    cache_policy_id          = data.aws_cloudfront_cache_policy.caching_disabled.id
    origin_request_policy_id = data.aws_cloudfront_origin_request_policy.all_viewer.id

    compress = false
  }

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  viewer_certificate {
    acm_certificate_arn      = aws_acm_certificate_validation.relaye.certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }
}

resource "aws_cloudfront_origin_access_control" "bevy_starter" {
  name                              = "bevy-starter"
  origin_access_control_origin_type = "s3"
  signing_behavior                  = "always"
  signing_protocol                  = "sigv4"
}

resource "aws_cloudfront_distribution" "bevy_starter" {
  enabled             = true
  comment             = "bevy-starter static bundle"
  default_root_object = "index.html"
  price_class         = "PriceClass_100"

  aliases = [local.bevy_starter_fqdn]

  origin {
    domain_name              = aws_s3_bucket.bevy_starter_static.bucket_regional_domain_name
    origin_id                = "s3-bevy-starter"
    origin_access_control_id = aws_cloudfront_origin_access_control.bevy_starter.id
  }

  default_cache_behavior {
    target_origin_id       = "s3-bevy-starter"
    viewer_protocol_policy = "redirect-to-https"

    allowed_methods = ["GET", "HEAD"]
    cached_methods  = ["GET", "HEAD"]

    cache_policy_id = data.aws_cloudfront_cache_policy.caching_optimized.id

    compress = true
  }

  restrictions {
    geo_restriction {
      restriction_type = "none"
    }
  }

  viewer_certificate {
    acm_certificate_arn      = aws_acm_certificate_validation.bevy_starter.certificate_arn
    ssl_support_method       = "sni-only"
    minimum_protocol_version = "TLSv1.2_2021"
  }
}
