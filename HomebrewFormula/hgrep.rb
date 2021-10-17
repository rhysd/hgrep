class Hgrep < Formula
  version '0.1.2'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
    sha256 'e71ffc728bb6b3770b9d4f429aeb82a1466205c5e30840931bcea27a933e5502' # mac
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 '76c365fcd09deced7f0c73bb2af8c9db3bdf5a7703984760f6b1ced02110b2d6' # linux
  end

  def install
    bin.install 'hgrep'
  end

  test do
    system "#{bin}/hgrep", '--version'
  end
end
