# Maintainer: Cameron Cross <cameroncros@gmail.com>
pkgname=PoorMansScreen
pkgver=0.0.1
pkgrel=1
pkgdesc="Very cut down version of screen"
arch=('x86_64' 'arm64')
url="https://github.com/cameroncros/PoorMansScreen"
license=('MIT' 'custom')
depends=()
makedepends=('rust')
source=("$pkgname-$pkgver.tar.gz::https://github.com/cameroncros/$pkgname/archive/$pkgver.tar.gz")

build() {
  cd "$pkgname-$pkgver"

  cargo build --release --locked
}

check() {
  cd "$pkgname-$pkgver"

  cargo test --release --locked
}

package() {
  cd "$pkgname-$pkgver"

  install -Dm755 "target/release/pms" "$pkgdir/usr/bin/pms"
}
