resource "aws_iam_user" "relaye" {
  name = "relaye"
  path = "/service/"
}

resource "aws_iam_user_policy" "relaye_box" {
  name = "relaye-box"
  user = aws_iam_user.relaye.name

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Sid    = "ArtifactsRead"
        Effect = "Allow"
        Action = ["s3:GetObject"]
        Resource = [
          "${aws_s3_bucket.relaye_artifacts.arn}/*",
        ]
      },
      {
        Sid    = "ParametersRead"
        Effect = "Allow"
        Action = [
          "ssm:GetParameter",
          "ssm:GetParameters",
        ]
        Resource = [
          aws_ssm_parameter.relaye_identity.arn,
          aws_ssm_parameter.relaye_atproto_client_key.arn,
        ]
      },
      {
        Sid    = "SsmKmsDecrypt"
        Effect = "Allow"
        Action = ["kms:Decrypt"]
        Resource = ["*"]
        Condition = {
          StringEquals = {
            "kms:ViaService" = "ssm.${var.aws_region}.amazonaws.com"
          }
        }
      },
    ]
  })
}

resource "aws_iam_access_key" "relaye" {
  user = aws_iam_user.relaye.name
}

# Stable identity for the relayer, persisted outside the instance so
# `aws_lightsail_instance.relaye` can be destroyed and re-created
# without minting a new PeerId. See variables.tf for how to seed
# `var.relaye_identity_bytes_b64` from the current running box.
resource "aws_ssm_parameter" "relaye_identity" {
  name        = "/relaye/identity-b64"
  description = "Base64 of the Ed25519 keypair backing relaye's libp2p PeerId."
  type        = "SecureString"
  value       = var.relaye_identity_bytes_b64
}

# P-256 keypair signing the atproto OAuth client_assertion JWTs. Public
# half committed as broker/jwks.json (kid = "laye-relaye-1"). Persisted
# outside the instance so a box rebuild does not invalidate the keypair
# advertised in JWKS.
resource "aws_ssm_parameter" "relaye_atproto_client_key" {
  name        = "/relaye/atproto-client-key-b64"
  description = "Base64 PKCS8 DER of the ES256 keypair that signs atproto OAuth client_assertion JWTs."
  type        = "SecureString"
  value       = var.relaye_atproto_client_key_b64
}
