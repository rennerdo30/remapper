pkgname="remapper"
pkgver="1.0.0"
pkgrel=1
pkgdesc='evdev remapping tool'
arch=('x86_64')
url="https://github.com/Rennerdo30/remapper"
license=('GPL3')
makedepends=('python' 'python-evdev')
source=("https://github.com/Rennerdo30/remapper/archive/master.zip")
sha256sums=('SKIP')

build() {
    cd "$srcdir/remapper-master"
    python setup.py build
}

package() {
    cd "$srcdir/remapper-master"
    mkdir -p "$pkgdir/usr/share/applications/"
    cp remapper.desktop "$pkgdir/usr/share/applications/"
    python setup.py install --root="$pkgdir" --optimize=1 --skip-build
}