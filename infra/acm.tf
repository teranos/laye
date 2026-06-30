resource "aws_acm_certificate" "relaye" {
  provider = aws.us_east_1

  domain_name       = local.relaye_fqdn
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_acm_certificate_validation" "relaye" {
  provider = aws.us_east_1

  certificate_arn         = aws_acm_certificate.relaye.arn
  validation_record_fqdns = [for r in aws_route53_record.relaye_cert_validation : r.fqdn]
}

resource "aws_acm_certificate" "bevy_starter" {
  provider = aws.us_east_1

  domain_name       = local.bevy_starter_fqdn
  validation_method = "DNS"

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_acm_certificate_validation" "bevy_starter" {
  provider = aws.us_east_1

  certificate_arn         = aws_acm_certificate.bevy_starter.arn
  validation_record_fqdns = [for r in aws_route53_record.bevy_starter_cert_validation : r.fqdn]
}
