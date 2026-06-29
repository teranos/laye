data "aws_route53_zone" "root" {
  name         = "${var.root_domain}."
  private_zone = false
}

resource "aws_route53_record" "origin_relaye" {
  zone_id = data.aws_route53_zone.root.zone_id
  name    = "origin-relaye.sbvh.nl"
  type    = "A"
  ttl     = 60
  records = [aws_lightsail_instance.relaye.public_ip_address]
}

resource "aws_route53_record" "relaye" {
  zone_id = data.aws_route53_zone.root.zone_id
  name    = local.relaye_fqdn
  type    = "A"

  alias {
    name                   = aws_cloudfront_distribution.relaye.domain_name
    zone_id                = aws_cloudfront_distribution.relaye.hosted_zone_id
    evaluate_target_health = false
  }
}

resource "aws_route53_record" "relaye_cert_validation" {
  for_each = {
    for dvo in aws_acm_certificate.relaye.domain_validation_options : dvo.domain_name => {
      name   = dvo.resource_record_name
      record = dvo.resource_record_value
      type   = dvo.resource_record_type
    }
  }

  zone_id         = data.aws_route53_zone.root.zone_id
  name            = each.value.name
  type            = each.value.type
  records         = [each.value.record]
  ttl             = 60
  allow_overwrite = true
}
