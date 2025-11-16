#!/usr/bin/env bash
# Color support for scripts
# Usage: source this file in your script

# Check if output is a TTY (terminal)
if [[ -t 1 ]]; then
  # Colors enabled
  COLOR_RESET='\033[0m'
  COLOR_BOLD='\033[1m'
  
  # Standard colors
  COLOR_RED='\033[0;31m'
  COLOR_GREEN='\033[0;32m'
  COLOR_YELLOW='\033[0;33m'
  COLOR_BLUE='\033[0;34m'
  COLOR_MAGENTA='\033[0;35m'
  COLOR_CYAN='\033[0;36m'
  COLOR_WHITE='\033[0;37m'
  
  # Bright colors
  COLOR_BRIGHT_RED='\033[1;31m'
  COLOR_BRIGHT_GREEN='\033[1;32m'
  COLOR_BRIGHT_YELLOW='\033[1;33m'
  COLOR_BRIGHT_BLUE='\033[1;34m'
  COLOR_BRIGHT_MAGENTA='\033[1;35m'
  COLOR_BRIGHT_CYAN='\033[1;36m'
else
  # Colors disabled (piping, redirecting, etc.)
  COLOR_RESET=''
  COLOR_BOLD=''
  COLOR_RED=''
  COLOR_GREEN=''
  COLOR_YELLOW=''
  COLOR_BLUE=''
  COLOR_MAGENTA=''
  COLOR_CYAN=''
  COLOR_WHITE=''
  COLOR_BRIGHT_RED=''
  COLOR_BRIGHT_GREEN=''
  COLOR_BRIGHT_YELLOW=''
  COLOR_BRIGHT_BLUE=''
  COLOR_BRIGHT_MAGENTA=''
  COLOR_BRIGHT_CYAN=''
fi

# Color functions for common use cases
color_info() {
  echo -e "${COLOR_CYAN}[info]${COLOR_RESET} $*"
}

color_success() {
  echo -e "${COLOR_GREEN}[success]${COLOR_RESET} $*"
}

color_warning() {
  echo -e "${COLOR_YELLOW}[warning]${COLOR_RESET} $*" >&2
}

color_error() {
  echo -e "${COLOR_RED}[error]${COLOR_RESET} $*" >&2
}

color_header() {
  echo -e "${COLOR_BOLD}${COLOR_BRIGHT_BLUE}$*${COLOR_RESET}"
}

color_step() {
  echo -e "${COLOR_BOLD}${COLOR_CYAN}→${COLOR_RESET} $*"
}

color_ok() {
  echo -e "${COLOR_GREEN}✓${COLOR_RESET} $*"
}

color_fail() {
  echo -e "${COLOR_RED}✗${COLOR_RESET} $*"
}
