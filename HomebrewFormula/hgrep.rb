class Hgrep < Formula
  version '0.3.8'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    if Hardware::CPU.intel?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
      sha256 'd248b3f3a82a5f13d05f0a202e9ccc41ebac9d621f64f4210b2a10bc60862c5c' # x86_64-apple-darwin
    end
    if Hardware::CPU.arm?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-aarch64-apple-darwin.zip"
      sha256 '869544e19fd49003f11255569dd38654fa947821b3131a0a098bcb18dc87530d' # aarch64-apple-darwin
    end
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 '88cabe76f592f224e2be35f2c8e2187fcf84224ca982eb3aff85f6cfd5db6b5a' # x86_64-unknown-linux-gnu
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
