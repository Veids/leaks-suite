#!/usr/bin/env bash
path=$1
mem_limit=$2
cpus=$3
split_size=$4

if [ $# -ne 4 ]; then
  echo "Not enough arguments"
  exit 1
fi

indexed_out_dir="./indexed"
sorted_out_dir="./sorted"
tmp_path="./tempo"
converted_dir="./converted"

function index(){
  input_path=$1
  indexed_out=$2
  indexed_error_out=$3
  ./indexer --input-type tar.gz -t ./public_suffix_list.dat -i "${input_path}" -o "${indexed_out}" -e "${indexed_error_out}"
}

function fail_on_rc(){
  rc=$1
  if [ $rc -ne 0 ]; then
    echo "Failed to process"
    exit $rc
  fi
}

file_name=$(basename "${path}")
indexed_out="${indexed_out_dir}/${file_name}.csv"
indexed_error_out="${indexed_out}.error.log"

echo "Indexing ${file_name}..."
index "${path}" "${indexed_out}" "${indexed_error_out}"
fail_on_rc $?

file_size=$(stat -c%s "${indexed_out}")
if [ $file_size -eq 0 ]; then
  echo "0 elements were obtained from indexing. Exiting..."
  rm "${indexed_out}"
  exit 0
fi

sorted_out="${sorted_out_dir}/${file_name}.s.csv"

echo "Sorting..."
sort -S "${mem_limit}" --parallel="${cpus}" -t ',' -k 1,1 "${indexed_out}" -T "${tmp_path}" > "${sorted_out}"
rc=$?
fail_on_rc $?
rm "${indexed_out}"

echo "Splitting by ${split_size}..."
split -C"${split_size}" "${sorted_out}" "${sorted_out_dir}/parts."
rc=$?
fail_on_rc $?
rm "${sorted_out}"

echo "Converting..."
i=0
for x in "${sorted_out_dir}/parts."*; do
  ./ctj -i "${x}" -o "${converted_dir}/${file_name}.${i}.jsonl"
  fail_on_rc $?

  rm "${x}"
  let 'i++'
done

echo "Done"
