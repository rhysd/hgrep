class Hgrep < Formula
  version '0.2.6'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    if Hardware::CPU.intel?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
      sha256 '255af1ea1fc743d882e3f4e55b1376cff864f0b3f433952fa3ff1f31f9fb024e' # x86_64-apple-darwin
    end
    if Hardware::CPU.arm?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-aarch64-apple-darwin.zip"
      sha256 '4db4c7a98c677cf1c660ed3c9d15a0d1e4928596a5790842c58003c05c33ebe8' # aarch64-apple-darwin
    end
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 '41d2662b5c1e7ca89845dd23c2c113f7facc62d93f4b3dbfd43a1ccbabc715f6' # x86_64-unknown-linux-gnu
  end

  def install
    bin.install 'hgrep'
    hgrep = bin/'hgrep'
    # Check if hgrep exists to avoid #6
    if hgrep.exist? && hgrep.executable?
      output = Utils.safe_popen_read(hgrep, '--generate-completion-script', 'zsh')
      (zsh_completion/'_hgrep').write output
      output = Utils.safe_popen_read(hgrep, '--generate-completion-script', 'bash')
      (bash_completion/'hgrep').write output
      output = Utils.safe_popen_read(hgrep, '--generate-completion-script', 'fish')
      (fish_completion/'hgrep.fish').write output
    end
  end

  test do
    system "#{bin}/hgrep", '--version'
  end
end
