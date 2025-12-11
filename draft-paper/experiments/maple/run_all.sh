filename=res.jsonl
rm $filename -f
./run_maple.sh && \
    ./run_jml.sh && \
    cat maple-recursive-eiffel/$filename >> $filename && \
    cat buggy-java-jml-eiffel/$filename >> $filename
