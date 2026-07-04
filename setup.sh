#!/bin/bash

#===============================================================================
# SentriX Attendance System - Development Setup Script
#===============================================================================
# This script sets up the development environment for the Attendance GEOTAG System
# Compatible with: Ubuntu/Debian Linux and macOS (Intel & Apple Silicon)
# 
# Features:
# - Installs Docker and Docker Compose (if not present)
# - Installs Node.js 24 LTS via nvm (if not present)
# - Checks existing versions and warns without overwriting
# - Installs project dependencies
# - Runs Docker Compose for development
# - Provides menu-driven interface
#===============================================================================

set -e  # Exit on any error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Global variables
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
NVM_VERSION="v0.40.5"
NODE_VERSION="22"  # Using Node 22 as required by backend/package.json
DOCKER_COMPOSE_VERSION=""  # Will be detected

# Detect OS
OS=""
LINUX_DISTRO=""
MAC_TYPE=""

detect_os() {
    case "$(uname -s)" in
        Darwin*)
            OS="mac"
            # Check if Apple Silicon
            if [[ "$(uname -m)" == "arm64" ]]; then
                MAC_TYPE="apple_silicon"
            else
                MAC_TYPE="intel"
            fi
            ;;
        Linux*)
            OS="linux"
            # Detect Linux distribution
            if [ -f /etc/os-release ]; then
                . /etc/os-release
                LINUX_DISTRO="${ID}"
            elif [ -f /etc/debian_version ]; then
                LINUX_DISTRO="debian"
            elif [ -f /etc/redhat-release ]; then
                LINUX_DISTRO="rhel"
            fi
            ;;
        CYGWIN*|MINGW*|MSYS*)
            OS="windows"
            ;;
        *)
            OS="unknown"
            ;;
    esac
}

# Print functions
print_banner() {
    echo ""
    echo -e "${PURPLE}╔════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${PURPLE}║      ${BOLD}${CYAN}SentriX Attendance System - Setup Script${NC}${PURPLE}           ║${NC}"
    echo -e "${PURPLE}║  ${YELLOW}Geotagged Attendance with Biometric Verification${NC}${PURPLE}       ║${NC}"
    echo -e "${PURPLE}╚════════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_section() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BOLD}${BLUE}  $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

print_info() {
    echo -e "${CYAN}ℹ${NC} $1"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_check() {
    echo -e "  ${PURPLE}→${NC} Checking: $1"
}

# Check if command exists
command_exists() {
    command -v "$1" &> /dev/null
}

# Get Node.js version (if installed)
get_node_version() {
    if command_exists node; then
        node --version 2>/dev/null | sed 's/v//'
    else
        echo ""
    fi
}

# Get npm version
get_npm_version() {
    if command_exists npm; then
        npm --version 2>/dev/null
    else
        echo ""
    fi
}

# Get Docker version
get_docker_version() {
    if command_exists docker; then
        docker --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1
    else
        echo ""
    fi
}

# Get Docker Compose version
get_docker_compose_version() {
    if docker compose version &>/dev/null; then
        docker compose version --short 2>/dev/null | sed 's/v//'
    elif command_exists docker-compose; then
        docker-compose --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1
    else
        echo ""
    fi
}

# Compare versions (returns 0 if version1 >= version2)
version_compare() {
    if [ "$(echo "$1 $2" | awk '{print ($1 >= $2)}')" = "1" ]; then
        return 0
    else
        return 1
    fi
}

# Ask for sudo if needed
ensure_sudo() {
    if [ "$OS" = "linux" ]; then
        if ! sudo -n true 2>/dev/null; then
            echo -e "${YELLOW}This operation requires sudo access.${NC}"
            sudo -v
        fi
    fi
}

#===============================================================================
# DOCKER INSTALLATION
#===============================================================================

install_docker_linux() {
    print_section "Installing Docker on Linux"
    ensure_sudo
    
    # Update package index
    print_info "Updating package index..."
    sudo apt-get update -qq
    
    # Install dependencies
    print_info "Installing dependencies..."
    sudo apt-get install -y -qq \
        ca-certificates \
        curl \
        gnupg \
        lsb-release \
        software-properties-common
    
    # Remove old Docker versions if any
    print_info "Removing old Docker installations..."
    sudo apt-get remove -y docker docker-engine docker.io containerd runc 2>/dev/null || true
    
    # Add Docker's official GPG key
    print_info "Adding Docker GPG key..."
    sudo install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/${LINUX_DISTRO}/gpg | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
    sudo chmod a+r /etc/apt/keyrings/docker.gpg
    
    # Add Docker repository
    print_info "Adding Docker repository..."
    echo \
        "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/${LINUX_DISTRO} \
        $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
    
    # Update package index again
    sudo apt-get update -qq
    
    # Install Docker Engine
    print_info "Installing Docker Engine, containerd, and Docker Compose..."
    sudo apt-get install -y -qq docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin
    
    # Add user to docker group
    print_info "Adding current user to docker group..."
    sudo usermod -aG docker "$USER"
    
    # Start Docker service
    print_info "Starting Docker service..."
    sudo systemctl enable docker
    sudo systemctl start docker
    
    print_success "Docker installed successfully!"
    
    if groups "$USER" | grep -q docker; then
        print_warning "Docker group membership requires a logout/login to take effect."
        print_warning "For now, you can run: newgrp docker"
    fi
}

install_docker_mac() {
    print_section "Installing Docker on macOS"
    
    # Check if Homebrew is installed
    if ! command_exists brew; then
        print_error "Homebrew is required but not installed."
        print_info "Installing Homebrew..."
        /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        
        # Add brew to PATH for Apple Silicon
        if [ "$MAC_TYPE" = "apple_silicon" ]; then
            echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zprofile
            eval "$(/opt/homebrew/bin/brew shellenv)"
        fi
    fi
    
    print_info "Installing Docker Desktop via Homebrew..."
    brew install --cask docker
    
    print_success "Docker Desktop installed!"
    print_info "Please open Docker Desktop from Applications to start Docker."
    print_info "After Docker starts, wait for it to initialize, then continue."
    
    read -p "$(echo -e ${CYAN}Press Enter once Docker Desktop is running...${NC})" -r
}

check_docker() {
    print_section "Checking Docker Installation"
    
    local docker_version
    docker_version=$(get_docker_version)
    
    if [ -n "$docker_version" ]; then
        print_check "Docker"
        print_success "Docker $docker_version is installed"
        
        # Verify Docker is running
        if docker info &>/dev/null; then
            print_success "Docker daemon is running"
        else
            print_warning "Docker is installed but not running"
            print_info "Starting Docker..."
            
            if [ "$OS" = "linux" ]; then
                sudo systemctl start docker
            elif [ "$OS" = "mac" ]; then
                print_info "Please start Docker Desktop manually"
                return 1
            fi
        fi
        return 0
    else
        print_check "Docker"
        print_warning "Docker is not installed"
        return 1
    fi
}

check_docker_compose() {
    print_section "Checking Docker Compose"
    
    local compose_version
    compose_version=$(get_docker_compose_version)
    
    if [ -n "$compose_version" ]; then
        print_check "Docker Compose"
        print_success "Docker Compose $compose_version is installed"
        DOCKER_COMPOSE_VERSION="$compose_version"
        return 0
    else
        print_check "Docker Compose"
        print_warning "Docker Compose is not installed"
        return 1
    fi
}

install_docker() {
    detect_os
    
    if [ "$OS" = "linux" ]; then
        install_docker_linux
    elif [ "$OS" = "mac" ]; then
        install_docker_mac
    else
        print_error "Unsupported OS: $(uname -s)"
        return 1
    fi
}

#===============================================================================
# NODE.JS INSTALLATION (via NVM)
#===============================================================================

install_nvm() {
    print_section "Installing NVM (Node Version Manager)"
    
    # Check if NVM is already installed
    if [ -d "$HOME/.nvm" ]; then
        print_warning "NVM is already installed at ~/.nvm"
        return 1
    fi
    
    print_info "Installing NVM $NVM_VERSION..."
    
    # Download and install NVM
    curl -o- "https://raw.githubusercontent.com/nvm-sh/nvm/${NVM_VERSION}/install.sh" | bash
    
    # Load NVM
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
    [ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion"
    
    print_success "NVM installed successfully!"
    return 0
}

install_node() {
    print_section "Installing Node.js"
    
    # Load NVM if available
    export NVM_DIR="$HOME/.nvm"
    if [ -s "$NVM_DIR/nvm.sh" ]; then
        \. "$NVM_DIR/nvm.sh"
    fi
    
    # Check if nvm is available
    if ! command_exists nvm && [ ! -s "$NVM_DIR/nvm.sh" ]; then
        print_info "Installing NVM first..."
        install_nvm
        \. "$NVM_DIR/nvm.sh"
    fi
    
    print_info "Installing Node.js $NODE_VERSION (LTS)..."
    
    # Install Node.js
    nvm install "$NODE_VERSION"
    nvm use "$NODE_VERSION"
    nvm alias default "$NODE_VERSION"
    
    # Verify installation
    local node_version
    node_version=$(node --version)
    local npm_version
    npm_version=$(npm --version)
    
    print_success "Node.js $node_version installed!"
    print_success "npm $npm_version installed!"
}

check_node() {
    print_section "Checking Node.js Installation"
    
    local node_version
    node_version=$(get_node_version)
    
    # Load NVM if available
    export NVM_DIR="$HOME/.nvm"
    if [ -s "$NVM_DIR/nvm.sh" ]; then
        \. "$NVM_DIR/nvm.sh"
        # Get current NVM version
        if nvm current &>/dev/null; then
            local nvm_current
            nvm_current=$(nvm current)
            print_info "NVM current version: $nvm_current"
        fi
    fi
    
    if [ -n "$node_version" ]; then
        print_check "Node.js"
        print_success "Node.js v${node_version} is installed"
        
        # Check version requirement (>= 22.0.0 from package.json)
        local major_version
        major_version=$(echo "$node_version" | cut -d. -f1)
        
        if [ "$major_version" -ge 22 ]; then
            print_success "Node.js version meets requirement (>= 22.0.0)"
            return 0
        else
            print_warning "Node.js version is below requirement"
            print_info "Required: Node.js >= 22.0.0"
            print_info "Found: Node.js $node_version"
            return 1
        fi
    else
        print_check "Node.js"
        print_warning "Node.js is not installed"
        return 1
    fi
}

check_npm() {
    print_section "Checking npm"
    
    local npm_version
    npm_version=$(get_npm_version)
    
    if [ -n "$npm_version" ]; then
        print_check "npm"
        print_success "npm $npm_version is installed"
        return 0
    else
        print_check "npm"
        print_warning "npm is not installed"
        return 1
    fi
}

check_nvm() {
    print_section "Checking NVM"
    
    export NVM_DIR="$HOME/.nvm"
    
    if [ -d "$NVM_DIR" ]; then
        print_check "NVM"
        print_success "NVM is installed at ~/.nvm"
        
        # Load NVM
        if [ -s "$NVM_DIR/nvm.sh" ]; then
            \. "$NVM_DIR/nvm.sh"
        
            # Get NVM version
            local nvm_version
            nvm_version=$(nvm --version)
            print_success "NVM version: $nvm_version"
            
            # List installed Node.js versions
            print_info "Installed Node versions:"
            nvm ls 2>/dev/null | head -10 || true
        fi
        return 0
    else
        print_check "NVM"
        print_warning "NVM is not installed"
        return 1
    fi
}

#===============================================================================
# PROJECT DEPENDENCIES
#===============================================================================

install_backend_deps() {
    print_section "Installing Backend Dependencies"
    
    cd "$SCRIPT_DIR/backend"
    
    # Load NVM if available
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
    
    if [ -f "package.json" ]; then
        print_info "Installing backend npm packages..."
        npm install
        print_success "Backend dependencies installed!"
    else
        print_error "package.json not found in backend/"
        return 1
    fi
}

install_frontend_deps() {
    print_section "Installing Frontend Dependencies"
    
    # Install admin frontend
    print_info "Installing admin frontend dependencies..."
    cd "$SCRIPT_DIR/frontend/admin"
    if [ -f "package.json" ]; then
        npm install
        print_success "Admin frontend dependencies installed!"
    fi
    
    # Install student frontend (minimal)
    print_info "Checking student frontend..."
    cd "$SCRIPT_DIR/frontend/student"
    if [ -f "package.json" ]; then
        npm install
        print_success "Student frontend dependencies installed!"
    else
        print_info "Student frontend uses static files (no npm dependencies)"
    fi
}

install_all_deps() {
    print_section "Installing All Project Dependencies"
    
    # Load NVM
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
    
    # Use correct Node version
    if command_exists nvm; then
        nvm use 2>/dev/null || nvm use "$NODE_VERSION"
    fi
    
    install_backend_deps
    install_frontend_deps
    
    print_success "All dependencies installed!"
}

check_deps() {
    print_section "Checking Project Dependencies"
    
    local backend_installed=false
    local admin_installed=false
    
    # Check backend
    if [ -d "$SCRIPT_DIR/backend/node_modules" ]; then
        print_check "Backend node_modules"
        print_success "Backend dependencies are installed"
        backend_installed=true
    else
        print_check "Backend node_modules"
        print_warning "Backend dependencies not installed"
    fi
    
    # Check admin frontend
    if [ -d "$SCRIPT_DIR/frontend/admin/node_modules" ]; then
        print_check "Admin frontend node_modules"
        print_success "Admin frontend dependencies are installed"
        admin_installed=true
    else
        print_check "Admin frontend node_modules"
        print_warning "Admin frontend dependencies not installed"
    fi
    
    if [ "$backend_installed" = true ] && [ "$admin_installed" = true ]; then
        return 0
    else
        return 1
    fi
}

#===============================================================================
# DOCKER COMPOSE OPERATIONS
#===============================================================================

copy_env_file() {
    print_section "Setting Up Environment File"
    
    if [ ! -f "$SCRIPT_DIR/.env" ]; then
        if [ -f "$SCRIPT_DIR/.env.example" ]; then
            print_info "Copying .env.example to .env"
            cp "$SCRIPT_DIR/.env.example" "$SCRIPT_DIR/.env"
            print_success ".env file created from .env.example"
            print_warning "Please edit .env with your actual configuration before running Docker Compose"
        else
            print_error ".env.example not found"
            return 1
        fi
    else
        print_success ".env file already exists"
    fi
}

docker_compose_up() {
    print_section "Starting Docker Compose"
    
    cd "$SCRIPT_DIR"
    
    # Check if .env exists
    if [ ! -f ".env" ]; then
        print_warning ".env file not found. Creating from .env.example..."
        copy_env_file
    fi
    
    print_info "Starting services with Docker Compose..."
    
    # Check Docker Compose availability
    if docker compose version &>/dev/null; then
        docker compose up -d --build
    elif command_exists docker-compose; then
        docker-compose up -d --build
    else
        print_error "Docker Compose not found"
        return 1
    fi
    
    print_success "Docker Compose started!"
    
    print_section "Service Status"
    docker compose ps 2>/dev/null || docker-compose ps
    
    echo ""
    print_info "Access the application at:"
    echo -e "  ${GREEN}Admin Panel:${NC}    http://localhost/admin"
    echo -e "  ${GREEN}Student Page:${NC}   http://localhost/attend/<token>"
    echo -e "  ${GREEN}API Health:${NC}     http://localhost/health"
}

docker_compose_down() {
    print_section "Stopping Docker Compose"
    
    cd "$SCRIPT_DIR"
    
    if docker compose version &>/dev/null; then
        docker compose down
    elif command_exists docker-compose; then
        docker-compose down
    else
        print_error "Docker Compose not found"
        return 1
    fi
    
    print_success "Docker Compose stopped!"
}

docker_compose_logs() {
    print_section "Viewing Docker Compose Logs"
    
    cd "$SCRIPT_DIR"
    
    if docker compose version &>/dev/null; then
        docker compose logs -f
    elif command_exists docker-compose; then
        docker-compose logs -f
    else
        print_error "Docker Compose not found"
        return 1
    fi
}

docker_compose_status() {
    print_section "Docker Compose Status"
    
    cd "$SCRIPT_DIR"
    
    if docker compose version &>/dev/null; then
        docker compose ps
        echo ""
        print_info "Container Health:"
        docker compose ps --format "table {{.Name}}\t{{.Status}}" 2>/dev/null || true
    elif command_exists docker-compose; then
        docker-compose ps
    else
        print_error "Docker Compose not found"
        return 1
    fi
}

docker_compose_reset() {
    print_section "Resetting Docker Compose (Removing Volumes)"
    
    cd "$SCRIPT_DIR"
    
    print_warning "This will remove all containers, volumes, and data!"
    echo -e "${RED}Are you sure? This cannot be undone. (y/N):${NC}"
    read -r
    echo
    
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        if docker compose version &>/dev/null; then
            docker compose down -v
        elif command_exists docker-compose; then
            docker-compose down -v
        fi
        print_success "Docker Compose reset complete!"
    else
        print_info "Reset cancelled"
    fi
}

#===============================================================================
# SYSTEM CHECKS
#===============================================================================

run_all_checks() {
    print_section "Running System Checks"
    
    detect_os
    print_info "Detected OS: ${OS} ${MAC_TYPE:-$LINUX_DISTRO}"
    echo ""
    
    local all_passed=true
    
    # Check Docker
    if check_docker; then
        :  # pass
    else
        all_passed=false
    fi
    
    # Check Docker Compose
    if check_docker_compose; then
        :  # pass
    else
        all_passed=false
    fi
    
    # Check Node.js
    check_nvm
    if check_node; then
        check_npm
    else
        all_passed=false
    fi
    
    # Check dependencies
    check_deps
    
    echo ""
    print_section "Check Summary"
    
    if $all_passed; then
        print_success "All required tools are installed!"
    else
        print_warning "Some tools are missing. Use the menu to install them."
    fi
}

#===============================================================================
# DEVELOPMENT SERVERS
#===============================================================================

run_backend_dev() {
    print_section "Starting Backend Development Server"
    
    cd "$SCRIPT_DIR/backend"
    
    # Load NVM
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
    
    # Set environment for development
    export NODE_ENV=development
    
    print_info "Starting backend in development mode..."
    print_info "Press Ctrl+C to stop"
    
    npm run dev
}

run_admin_dev() {
    print_section "Starting Admin Frontend Development Server"
    
    cd "$SCRIPT_DIR/frontend/admin"
    
    # Load NVM
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
    
    print_info "Starting Vite dev server..."
    print_info "Press Ctrl+C to stop"
    
    npm run dev
}

#===============================================================================
# TESTS
#===============================================================================

run_backend_tests() {
    print_section "Running Backend Tests"
    
    cd "$SCRIPT_DIR/backend"
    
    # Load NVM
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
    
    print_info "Running Jest tests..."
    npm test
}

run_backend_lint() {
    print_section "Running Backend Linter"
    
    cd "$SCRIPT_DIR/backend"
    
    # Load NVM
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
    
    print_info "Running ESLint..."
    npm run lint
}

#===============================================================================
# MENU SYSTEM
#===============================================================================

show_main_menu() {
    clear
    print_banner
    print_section "Main Menu"
    
    echo -e "  ${CYAN}1)${NC} Run System Checks"
    echo -e "  ${CYAN}2)${NC} Install Docker & Docker Compose"
    echo -e "  ${CYAN}3)${NC} Install Node.js (via NVM)"
    echo -e "  ${CYAN}4)${NC} Install Project Dependencies"
    echo -e "  ${CYAM}5)${NC} Copy .env File"
    echo ""
    echo -e "  ${GREEN}6)${NC} Start Docker Compose (Production)"
    echo -e "  ${GREEN}7)${NC} Stop Docker Compose"
    echo -e "  ${GREEN}8)${NC} View Docker Logs"
    echo -e "  ${GREEN}9)${NC} Docker Compose Status"
    echo -e "  ${YELLOW}R)${NC} Reset Docker (Remove Volumes)"
    echo ""
    echo -e "  ${PURPLE}10)${NC} Run Backend Dev Server (nodemon)"
    echo -e "  ${PURPLE}11)${NC} Run Admin Frontend Dev (Vite)"
    echo ""
    echo -e "  ${BLUE}12)${NC} Run Backend Tests"
    echo -e "  ${BLUE}13)${NC} Run Backend Linter"
    echo ""
    echo -e "  ${RED}Q)${NC} Quit"
    echo ""
    echo -n -e "${BOLD}Select an option:${NC} "
}

handle_menu() {
    local choice
    read -r choice
    
    case $choice in
        1)
            run_all_checks
            ;;
        2)
            detect_os
            if [ "$OS" = "linux" ] || [ "$OS" = "mac" ]; then
                install_docker
            else
                print_error "Unsupported OS: $OS"
            fi
            ;;
        3)
            install_nvm
            # Source nvm
            export NVM_DIR="$HOME/.nvm"
            [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
            install_node
            ;;
        4)
            install_all_deps
            ;;
        5)
            copy_env_file
            ;;
        6)
            docker_compose_up
            ;;
        7)
            docker_compose_down
            ;;
        8)
            docker_compose_logs
            ;;
        9)
            docker_compose_status
            ;;
        [rR])
            docker_compose_reset
            ;;
        10)
            run_backend_dev
            ;;
        11)
            run_admin_dev
            ;;
        12)
            run_backend_tests
            ;;
        13)
            run_backend_lint
            ;;
        [qQ])
            echo -e "${GREEN}Goodbye!${NC}"
            exit 0
            ;;
        *)
            print_error "Invalid option"
            ;;
    esac
    
    echo ""
    echo -e "${YELLOW}Press Enter to continue...${NC}"
    read -r
}

#===============================================================================
# QUICK MODE (COMMAND LINE ARGUMENTS)
#===============================================================================

print_usage() {
    echo ""
    echo -e "${BOLD}Usage:${NC} ./setup.sh [option]"
    echo ""
    echo -e "${CYAN}Options:${NC}"
    echo "  check       Run system checks"
    echo "  install     Install all (Docker, Node.js, Dependencies)"
    echo "  docker      Install Docker only"
    echo "  node        Install Node.js only (via NVM)"
    echo "  deps        Install project dependencies only"
    echo "  env         Copy .env.example to .env"
    echo "  up          Start Docker Compose"
    echo "  down        Stop Docker Compose"
    echo "  logs        View Docker Compose logs"
    echo "  status      Show Docker Compose status"
    echo "  reset       Reset Docker (remove volumes)"
    echo "  test        Run backend tests"
    echo "  lint        Run backend linter"
    echo "  dev         Run backend dev server"
    echo "  menu        Show interactive menu (default)"
    echo "  help        Show this help message"
    echo ""
    echo -e "${YELLOW}Examples:${NC}"
    echo "  ./setup.sh check"
    echo "  ./setup.sh install"
    echo "  ./setup.sh up"
    echo ""
}

handle_cli() {
    local option="$1"
    
    case $option in
        check)
            run_all_checks
            ;;
        install)
            detect_os
            if [ "$OS" = "linux" ] || [ "$OS" = "mac" ]; then
                install_docker
            fi
            install_nvm
            export NVM_DIR="$HOME/.nvm"
            [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
            install_node
            install_all_deps
            copy_env_file
            ;;
        docker)
            detect_os
            install_docker
            ;;
        node)
            install_nvm
            export NVM_DIR="$HOME/.nvm"
            [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
            install_node
            ;;
        deps)
            install_all_deps
            ;;
        env)
            copy_env_file
            ;;
        up)
            docker_compose_up
            ;;
        down)
            docker_compose_down
            ;;
        logs)
            docker_compose_logs
            ;;
        status)
            docker_compose_status
            ;;
        reset)
            docker_compose_reset
            ;;
        test)
            run_backend_tests
            ;;
        lint)
            run_backend_lint
            ;;
        dev)
            run_backend_dev
            ;;
        help|--help|-h)
            print_usage
            exit 0
            ;;
        menu|"")
            # Show interactive menu
            while true; do
                show_main_menu
                handle_menu
            done
            ;;
        *)
            print_error "Unknown option: $option"
            print_usage
            exit 1
            ;;
    esac
}

#===============================================================================
# MAIN
#===============================================================================

main() {
    # Detect OS
    detect_os
    
    # Check if running on supported OS
    if [ "$OS" = "unknown" ] || [ "$OS" = "windows" ]; then
        print_banner
        print_error "Unsupported operating system: $(uname -s)"
        print_info "This script supports Linux (Ubuntu/Debian) and macOS"
        exit 1
    fi
    
    # Handle command line argument
    handle_cli "${1:-menu}"
}

# Run main
main "$@"
