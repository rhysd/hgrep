class Hgrep < Formula
  version '0.3.3'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    if Hardware::CPU.intel?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
      sha256 'eec68e4f8d0c39913f3ebf3177ac475e80397cfb93499986d41f5e0dcf386968' # x86_64-apple-darwin
    end
    if Hardware::CPU.arm?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-aarch64-apple-darwin.zip"
      sha256 '22551dc33a83c29c723d6500cd2b589d4705e71428a4a004f40224d4fc413c72' # aarch64-apple-darwin
    end
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 '3f1a3075312d44317f7d5000150ef0290c8faf1b6ebb90a7414fef1fac37ab0e' # x86_64-unknown-linux-gnu
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
      output = Utils.safe_popen_read(hgrep, '--generate-man-page')
      (man1/'hgrep.1').write output
    end
  end

  test do
    system "#{bin}/hgrep", '--version'
  end
end
