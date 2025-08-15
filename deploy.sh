#!/bin/bash
set -e

# Lead Scraper API Deployment Script
# Usage: ./deploy.sh

echo "🚀 Starting Lead Scraper API deployment..."

# Configuration
APP_NAME="lead-scraper"
APP_DIR="/var/www/lead-scraper"
NGINX_SITE="lead-scraper"
SERVICE_USER="www-data"
DOMAIN="lead.mayorana.ch"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Functions
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    log_error "Please run as root (use sudo)"
    exit 1
fi

# Update system packages
log_info "Updating system packages..."
apt update && apt upgrade -y

# Install required packages
log_info "Installing required packages..."
apt install -y nginx certbot python3-certbot-nginx curl build-essential

# Install Node.js and PM2
log_info "Installing Node.js and PM2..."
curl -fsSL https://deb.nodesource.com/setup_lts.x | bash -
apt install -y nodejs
npm install -g pm2

# Install Rust (if not already installed)
if ! command -v rustc &> /dev/null; then
    log_info "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source ~/.cargo/env
    rustup default stable
else
    log_info "Rust already installed"
fi

# Create application directory
log_info "Creating application directory..."
mkdir -p $APP_DIR
mkdir -p $APP_DIR/data
mkdir -p $APP_DIR/out
mkdir -p $APP_DIR/public
mkdir -p /var/log/pm2

# Copy application files
log_info "Copying application files..."
cp -r . $APP_DIR/
cd $APP_DIR

# Build the application
log_info "Building application in release mode..."
cargo build --release

# Set permissions
log_info "Setting permissions..."
chown -R $SERVICE_USER:$SERVICE_USER $APP_DIR
chmod +x $APP_DIR/target/release/lead-scraper

# Create environment file
log_info "Creating environment file..."
cat > $APP_DIR/.env << EOF
# GitHub API
GITHUB_TOKEN=your_github_token_here

# Email Configuration  
MAILGUN_API_KEY=your_mailgun_api_key_here
MAILGUN_DOMAIN=your_mailgun_domain_here
FROM_EMAIL=support@$DOMAIN
FROM_NAME=Lead Scraper
CONTACT_EMAIL=contact@$DOMAIN
CONTACT_PHONE=+41000000000

# Debug mode (set to false in production)
EMAIL_DEBUG_MODE=false
EMAIL_DEBUG_ADDRESS=debug@$DOMAIN

# Rocket Configuration
ROCKET_ENV=production
ROCKET_ADDRESS=127.0.0.1
ROCKET_PORT=8001
RUST_LOG=lead_scraper=info,rocket=warn
EOF

log_warn "Please edit $APP_DIR/.env with your actual configuration values"

# Setup PM2 for the application
log_info "Setting up PM2..."
cd $APP_DIR
pm2 stop $APP_NAME 2>/dev/null || true
pm2 delete $APP_NAME 2>/dev/null || true
pm2 start ecosystem.config.js
pm2 save
pm2 startup

# Configure nginx
log_info "Configuring nginx..."
cp nginx-lead-scraper.conf /etc/nginx/sites-available/$NGINX_SITE

# Enable the site
ln -sf /etc/nginx/sites-available/$NGINX_SITE /etc/nginx/sites-enabled/
rm -f /etc/nginx/sites-enabled/default

# Test nginx configuration
log_info "Testing nginx configuration..."
nginx -t

if [ $? -eq 0 ]; then
    log_info "Nginx configuration is valid"
    systemctl reload nginx
    systemctl enable nginx
else
    log_error "Nginx configuration is invalid"
    exit 1
fi

# Create a simple index.html for the root
cat > $APP_DIR/public/index.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Lead Scraper API</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; background: #f5f5f5; }
        .container { max-width: 800px; margin: 0 auto; background: white; padding: 40px; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
        .endpoint { background: #f4f4f4; padding: 15px; margin: 10px 0; border-radius: 5px; border-left: 4px solid #007acc; }
        .method { color: #007acc; font-weight: bold; }
        .status { background: #e8f5e8; color: #2d5a2d; padding: 5px 10px; border-radius: 3px; display: inline-block; margin-bottom: 20px; }
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 Lead Scraper API</h1>
        <div class="status">✅ API Server Running</div>
        
        <p>The Lead Scraper API is running and ready to serve requests.</p>
        
        <h2>Available Endpoints</h2>
        
        <div class="endpoint">
            <span class="method">GET</span> <strong>/api/stats</strong><br>
            General database statistics and overview
        </div>
        
        <div class="endpoint">
            <span class="method">GET</span> <strong>/api/projects</strong><br>
            List GitHub projects with pagination and filtering
        </div>
        
        <div class="endpoint">
            <span class="method">GET</span> <strong>/api/leads</strong><br>
            List email leads and contacts
        </div>
        
        <div class="endpoint">
            <span class="method">GET</span> <strong>/api/companies</strong><br>
            List discovered companies
        </div>
        
        <div class="endpoint">
            <span class="method">GET</span> <strong>/api/sources</strong><br>
            List scraped awesome sources
        </div>
        
        <div class="endpoint">
            <span class="method">GET</span> <strong>/health</strong><br>
            API health check endpoint
        </div>
        
        <p><strong>Quick Test:</strong> <a href="/api/stats" target="_blank">→ Test API Stats Endpoint</a></p>
        
        <hr style="margin: 30px 0;">
        <p style="color: #666; font-size: 14px;">
            Lead Scraper API • Domain: lead.mayorana.ch
        </p>
    </div>
</body>
</html>
EOF

# Setup log rotation
log_info "Setting up log rotation..."
cat > /etc/logrotate.d/lead-scraper << EOF
/var/log/pm2/lead-scraper-*.log {
    daily
    missingok
    rotate 7
    compress
    notifempty
    create 644 $SERVICE_USER $SERVICE_USER
    postrotate
        pm2 reloadLogs
    endscript
}

/var/log/nginx/lead-scraper-*.log {
    daily
    missingok
    rotate 14
    compress
    notifempty
    create 644 www-data www-data
    postrotate
        systemctl reload nginx
    endscript
}
EOF

# Create a simple status check script
cat > $APP_DIR/status.sh << 'EOF'
#!/bin/bash
echo "=== Lead Scraper API Status ==="
echo "PM2 Status:"
pm2 status lead-scraper-api

echo -e "\nNginx Status:"
systemctl status nginx --no-pager -l

echo -e "\nAPI Health Check:"
curl -s http://localhost:8001/api/stats | head -c 200
echo -e "\n"

echo -e "\nDisk Usage:"
df -h /var/www/lead-scraper

echo -e "\nRecent Logs:"
tail -n 5 /var/log/pm2/lead-scraper-api.log
EOF

chmod +x $APP_DIR/status.sh

log_info "✅ Deployment completed successfully!"
log_info ""
log_info "Next steps:"
log_info "1. Edit $APP_DIR/.env with your actual configuration"
log_info "2. Setup SSL: sudo certbot --nginx -d $DOMAIN"
log_info "3. Test the API: curl http://$DOMAIN/api/stats"
log_info "4. Check status: $APP_DIR/status.sh"
log_info ""
log_info "Useful commands:"
log_info "- View logs: pm2 logs lead-scraper-api"
log_info "- Restart API: pm2 restart lead-scraper-api"
log_info "- Reload nginx: sudo systemctl reload nginx"
log_info "- Check status: $APP_DIR/status.sh"
log_info "- Setup SSL: sudo certbot --nginx -d $DOMAIN"_info "- Restart API: pm2 restart lead-scraper-api"
log_info "- Reload nginx: systemctl reload nginx"
log_info "- Check status: $APP_DIR/status.sh"
