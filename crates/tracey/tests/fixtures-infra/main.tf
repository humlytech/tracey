# r[impl infra.bucket.versioning]
resource "aws_s3_bucket_versioning" "logs" {
  bucket = aws_s3_bucket.logs.id

  versioning_configuration {
    status = "Enabled" // r[verify infra.bucket.versioning]
  }
}
