class Hgrep < Formula
  version '0.1.8'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    if Hardware::CPU.intel?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
      sha256 'b57d18e7b46071b327a83537d7dac714af3122647aedf47b00faede7ce5f3bc0' # mac_x86_64
    end
    if Hardware::CPU.arm?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-aarch64-apple-darwin.zip"
      sha256 '0019dfc4b32d63c1392aa264aed2253c1e0c2fb09216f8e2cc269bbfb8bb49b5' # mac_aarch64
    end
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 '0d78a9cd05216850867f9b6b0659ac8c0dd4da2ee18234f607189b8cfeb909ef' # linux
  end

  def install
    bin.install 'hgrep'
  end

  test do
    system "#{bin}/hgrep", '--version'
  end
end
