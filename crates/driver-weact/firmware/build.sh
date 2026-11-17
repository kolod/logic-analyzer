#!/bin/sh
# Builds the fx2lafw image for the sigrok-fx2-8ch board with SDCC, without autotools.
#
# usage: build.sh <source-dir> <output.fw> [<patch>]
#
# <source-dir> is a checkout of sigrok-firmware-fx2lafw (the vendored fx2lib is inside it). It needs
# `sdcc`, `sdas8051`, `sdar` and `makebin` (SDCC 3.4 or newer) on the PATH. The steps are those of
# the project's Makefile.am for hw/sigrok-fx2-8ch. With <patch> (for example `fx2lafw-stop.patch`) the
# patch is applied to the checkout first. Use a scratch checkout: the build edits files in place.
set -eu
src=$1
out=$(cd "$(dirname "$2")" && pwd)/$(basename "$2")
patch=${3:+$(cd "$(dirname "$3")" && pwd)/$(basename "$3")}
cd "$src"
if [ -n "$patch" ]; then git apply "$patch"; fi
SDCC=${SDCC:-sdcc}
SDAS=${SDAS:-sdas8051}
SDAR=${SDAR:-sdar}
MAKEBIN=${MAKEBIN:-makebin}
cinc="-Iinclude -Ifx2lib/include"

# SDCC 4 wants the interrupt number in parentheses and the address of a bit as a parenthesised expression, the SDCC 3 of the project's
# era took `__at 0x88+1 IE0`. This changes the build copy only, never the shipped source.
sed -i -E 's/__at (0x[0-9A-Fa-f]+) ?\+ ?([0-9]+) /__at (\1+\2) /' fx2lib/include/fx2regs.h
sed -i -E 's/__interrupt ([A-Z_0-9]+)/__interrupt(\1)/' fx2lib/include/*.h fx2lib/lib/*.c fx2lib/lib/interrupts/*.c fx2lafw.c gpif-acquisition.c

lib_objs=""
for f in delay eputils gpif i2c serial setupdat; do
	$SDCC -mmcs51 $cinc -c fx2lib/lib/$f.c -o fx2lib/lib/$f.rel
	lib_objs="$lib_objs fx2lib/lib/$f.rel"
done
for f in int4av usbav; do
	$SDAS -glos -Iinclude fx2lib/lib/$f.rel fx2lib/lib/$f.a51
	lib_objs="$lib_objs fx2lib/lib/$f.rel"
done
rm -f fx2lib/lib/fx2.lib
$SDAR -rc fx2lib/lib/fx2.lib $lib_objs

int_objs=""
for c in fx2lib/lib/interrupts/*.c; do
	r=${c%.c}.rel
	$SDCC -mmcs51 $cinc -c "$c" -o "$r"
	int_objs="$int_objs $r"
done
rm -f fx2lib/lib/interrupts/ints.lib
$SDAR -rc fx2lib/lib/interrupts/ints.lib $int_objs

$SDAS -glos -Iinclude hw/sigrok-fx2-8ch/dscr.rel hw/sigrok-fx2-8ch/dscr.a51
$SDCC -mmcs51 $cinc -c fx2lafw.c -o fx2lafw.rel
$SDCC -mmcs51 $cinc -c gpif-acquisition.c -o gpif-acquisition.rel

$SDCC -mmcs51 --code-size 0x1c00 --xram-size 0x0200 --xram-loc 0x1c00 \
	-Wl"-b DSCR_AREA=0x1e00" -Wl"-b INT2JT=0x1f00" \
	-o fx2lafw.ihx hw/sigrok-fx2-8ch/dscr.rel fx2lafw.rel gpif-acquisition.rel \
	fx2lib/lib/fx2.lib fx2lib/lib/interrupts/ints.lib
$MAKEBIN -p < fx2lafw.ihx > "$out"
ls -l "$out"
