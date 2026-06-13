#!/bin/bash
# Generate self-signed TLS certificates for Zenth DHT server
# For development/testing only - use proper CA-signed certs in production

set -e

CERT_DIR="certs"
DAYS_VALID=365
KEY_SIZE=4096

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Zenth DHT TLS Certificate Generator ===${NC}"
echo ""

# Create certs directory if it doesn't exist
mkdir -p "$CERT_DIR"

# Check if certificates already exist
if [ -f "$CERT_DIR/cert.pem" ] && [ -f "$CERT_DIR/key.pem" ]; then
    echo -e "${YELLOW}Certificates already exist in $CERT_DIR/${NC}"
    read -p "Do you want to regenerate them? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Keeping existing certificates."
        exit 0
    fi
fi

echo "Generating RSA $KEY_SIZE-bit private key..."
openssl genrsa -out "$CERT_DIR/key.pem" $KEY_SIZE 2>/dev/null

echo "Generating self-signed certificate (valid for $DAYS_VALID days)..."

# Generate certificate with proper extensions for TLS 1.3
openssl req -new -x509 \
    -key "$CERT_DIR/key.pem" \
    -out "$CERT_DIR/cert.pem" \
    -days $DAYS_VALID \
    -subj "/C=FR/ST=France/L=Paris/O=Zenth/OU=DHT/CN=localhost" \
    -addext "subjectAltName=DNS:localhost,DNS:*.localhost,IP:127.0.0.1,IP:::1" \
    -addext "keyUsage=digitalSignature,keyEncipherment" \
    -addext "extendedKeyUsage=serverAuth" \
    2>/dev/null

# Set proper permissions
chmod 600 "$CERT_DIR/key.pem"
chmod 644 "$CERT_DIR/cert.pem"

echo ""
echo -e "${GREEN}Certificates generated successfully!${NC}"
echo ""
echo "Files created:"
echo "  - $CERT_DIR/cert.pem (certificate)"
echo "  - $CERT_DIR/key.pem (private key)"
echo ""
echo "Certificate details:"
openssl x509 -in "$CERT_DIR/cert.pem" -noout -subject -dates -ext subjectAltName 2>/dev/null || true
echo ""
echo -e "${YELLOW}WARNING: These are self-signed certificates for development only.${NC}"
echo -e "${YELLOW}For production, use certificates from a trusted CA (Let's Encrypt, etc.)${NC}"
echo ""
echo "To use with the server, set these environment variables:"
echo "  export TLS_CERT_PATH=$CERT_DIR/cert.pem"
echo "  export TLS_KEY_PATH=$CERT_DIR/key.pem"
echo ""
echo "Or add them to your .env file."
