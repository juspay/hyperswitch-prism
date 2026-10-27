# Source me before starting a GRACE session on macOS:  . ~/grace-darwin/env.sh
#
# Order matters. The shim dir must precede Homebrew's gnubin so our `readlink`
# (which also answers /proc/<pid>/{exe,cwd,fd/N}) wins over coreutils' plain
# one, and gnubin must precede /usr/bin so GNU `stat -c`, `date -d`,
# `realpath -m`, `sed -i` and `xargs -d` win over the BSD versions.

export GRACE_DARWIN="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"

PATH="$GRACE_DARWIN"                                   # ss, readlink(/proc)
PATH="$PATH:/opt/homebrew/opt/coreutils/libexec/gnubin"    # stat date realpath nproc timeout du df base64 sha256sum
PATH="$PATH:/opt/homebrew/opt/gnu-sed/libexec/gnubin"      # sed -i without a suffix
PATH="$PATH:/opt/homebrew/opt/findutils/libexec/gnubin"    # find xargs -d
PATH="$PATH:/opt/homebrew/opt/util-linux/bin"              # flock setsid
PATH="$PATH:/opt/homebrew/opt/util-linux/sbin"
PATH="$PATH:$HOME/.cargo/bin"                              # rustup shim, so cargo +nightly works
PATH="$PATH:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export PATH

# `tr -dc` over /dev/urandom aborts with "Illegal byte sequence" under a UTF-8
# locale; 2.6a generates HS_WEBHOOK_SECRET and HS_ADMIN_API_KEY that way.
export LC_ALL=C

# Rust needs a bigger stack for the deep connector type trees, and the workflow
# sets this itself when it starts servers -- set it here too so ad-hoc builds match.
export RUST_MIN_STACK=16777216

grace_doctor() {
  local miss=""
  for t in git cargo rustup grpcurl jq typos gh ss curl flock nc python3 \
           nproc setsid timeout realpath openssl; do
    command -v "$t" >/dev/null || miss="$miss $t"
  done
  [ -z "$miss" ] && echo "tools: all present" || echo "tools MISSING:$miss"
  stat -c %Y . >/dev/null 2>&1 && echo "gnu stat: ok"       || echo "gnu stat: FAIL"
  date -u -d '+1 hour' >/dev/null 2>&1 && echo "gnu date: ok" || echo "gnu date: FAIL"
  realpath -m /nope/x  >/dev/null 2>&1 && echo "gnu realpath: ok" || echo "gnu realpath: FAIL"
  ss -Htln >/dev/null 2>&1 && echo "ss shim: ok"            || echo "ss shim: FAIL"
  cargo +nightly fmt --version >/dev/null 2>&1 && echo "nightly rustfmt: ok" || echo "nightly rustfmt: FAIL"
  echo "avail: $(df -Pk . | awk 'NR==2{print int($4/1048576)}') GB"
}
