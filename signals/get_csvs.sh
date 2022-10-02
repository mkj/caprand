#!/usr/bin/env zsh

fn="$1"

function ex() {
	dir="$1"
	fn="$2"

	(echo t_$dir,imm_$dir,drive_$dir; grep t_$dir "$fn" | tail -n +200 | head -n 100000 | cut -d ' ' -f 5,6,9|sed 's/ /,/g' ) > "$fn.$dir"
}

ex down "$fn"
ex up "$fn"


