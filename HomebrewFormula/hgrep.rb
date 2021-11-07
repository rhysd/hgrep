class Hgrep < Formula
  version '0.2.0'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    if Hardware::CPU.intel?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
      sha256 '319da7b3f4e8c2939ace70840c85172f54fd977dbb3661f75433ea5f7a1615af' # mac_x86_64
    end
    if Hardware::CPU.arm?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-aarch64-apple-darwin.zip"
      sha256 '4c9ffd5607461b720f6be55cb7ce9e85eb55a7b6900a6a8d47f63d97a126a22f' # mac_aarch64
    end
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 '320c0f9e274501223e394be5cb2138a0ee6698ea900ff303c4aa343530fbb1ca' # linux
  end

  def install
    bin.install 'hgrep'
  end

  test do
    system "#{bin}/hgrep", '--version'
  end
end
