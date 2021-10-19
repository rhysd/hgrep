class Hgrep < Formula
  version '0.1.3'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
    sha256 'eb7649b0d7e44d6433445a8923beb48617e38fc768e72ad658c2900be07d7c6b' # mac
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 '3c920e02fc19804c2092782b680f47a46944e580e3dffe3ebcbedcf0e15b9021' # linux
  end

  def install
    bin.install 'hgrep'
  end

  test do
    system "#{bin}/hgrep", '--version'
  end
end
