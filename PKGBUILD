# Maintainer: Cameron Cross <cameroncros@gmail.com>
pkgname=PoorMansScreen
pkgver=0.0.1
pkgrel=1
pkgdesc="Very cut down version of screen"
arch=('x86_64' 'arm64')
url="https://github.com/cameroncros/PoorMansScreen"
license=('MIT' 'custom')
depends=()
makedepends=('rust' 'git')
source=("$pkgname::git+https://github.com/cameroncros/$pkgname#branch=main")
sha256sums=('SKIP')

build() {
  cd "$pkgname"

  cargo build --release --locked
}

check() {
  cd "$pkgname"

  cargo test --release --locked
}

package() {
  cd "$pkgname"

  install -Dm755 "target/release/pms" "$pkgdir/usr/bin/pms"
}
