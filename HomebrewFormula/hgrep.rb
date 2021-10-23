class Hgrep < Formula
  version '0.1.6'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    if Hardware::CPU.intel?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
      sha256 'e1f47ffbf828ea97344342343fcc57ea218a267d2d682048750e04914bd1448d' # mac_x86_64
    end
    if Hardware::CPU.arm?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-aarch64-apple-darwin.zip"
      sha256 '0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5' # mac_aarch64
    end
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 'a02d12ce70ed647eb82fc1fb439c44fcd27b74bdb2ad9ed06029cf0ae93d183a' # linux
  end

  def install
    bin.install 'hgrep'
  end

  test do
    system "#{bin}/hgrep", '--version'
  end
end
