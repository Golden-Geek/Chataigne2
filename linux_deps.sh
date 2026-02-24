sudo apt update

#rust
sudo apt install -y build-essential pkg-config libsoup-3.0-devl ibatk1.0-dev libgdk-pixbuf-2.0-dev libcairo2-dev libpango1.0-dev libgtk-3-dev libwebkitgtk-6.0-dev libwebkit2gtk-4.1-dev

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

#node
# Download and install nvm:
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.4/install.sh | bash

# in lieu of restarting the shell
\. "$HOME/.nvm/nvm.sh"

# Download and install Node.js:
nvm install 25

# Verify the Node.js version:
node -v # Should print "v25.6.1".

# Verify npm version:
npm -v # Should print "11.9.0".


