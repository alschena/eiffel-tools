filename=res.jsonl
dir=buggy-java-jml-eiffel
cd $dir && rm $filename -f && \
    clear && \
    cargo build --release -p llm-correct-features && \
    \
    git restore . && \
    clear && \
    cd .. && \
    python buggy_prepare.py $dir > $dir/classes.txt && \
    cd $dir && \
    \
    ../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt --model "gpt-4.1-nano" | tee -a $filename
