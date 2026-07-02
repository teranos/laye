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
        Sid      = "IdentitySecretRead"
        Effect   = "Allow"
        Action   = ["secretsmanager:GetSecretValue"]
        Resource = [aws_secretsmanager_secret.relaye_identity.arn]
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
resource "aws_secretsmanager_secret" "relaye_identity" {
  name                    = "relaye/identity-b64"
  description             = "Base64 of the Ed25519 keypair backing relaye's libp2p PeerId."
  recovery_window_in_days = 7
}

resource "aws_secretsmanager_secret_version" "relaye_identity" {
  secret_id     = aws_secretsmanager_secret.relaye_identity.id
  secret_string = var.relaye_identity_bytes_b64
}
