class Hgrep < Formula
  version '0.1.9'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    if Hardware::CPU.intel?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
      sha256 'b8bd6d271c4d2c6ef6d9716d79f8195c9a3bd2effe6f6507c30d25fb6fbe2f40' # mac_x86_64
    end
    if Hardware::CPU.arm?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-aarch64-apple-darwin.zip"
      sha256 '0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5' # mac_aarch64
    end
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 '819ae3d2ea73275fd203477982b6cfb73095fe3a18983bf15b4e367cee6432f7' # linux
  end

  def install
    bin.install 'hgrep'
  end

  test do
    system "#{bin}/hgrep", '--version'
  end
end
