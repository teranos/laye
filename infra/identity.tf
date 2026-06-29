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
    ]
  })
}

resource "aws_iam_access_key" "relaye" {
  user = aws_iam_user.relaye.name
}
