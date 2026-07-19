openssl pkey -in private_key.pem -pubout -out public_key.pem
chmod 600 private_key.pem
# Ensure private key is not accidentally committed to git
if grep -q "private_key.pem" .gitignore 2>/dev/null; then
    echo "private_key.pem is already protected by .gitignore"
else
    echo "WARNING: private_key.pem may not be in .gitignore!"
fi
