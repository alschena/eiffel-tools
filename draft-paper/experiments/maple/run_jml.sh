filename=res.jsonl
cd buggy-java-jml-eiffel/ && rm $filename -f && \
    clear && \
    cargo build --release -p llm-correct-features && \
    \
    git restore . && \
    clear && \
    cd .. && \
    python buggy_prepare.py > buggy-java-jml-eiffel/classes.txt && \
    cd buggy-java-jml-eiffel && \
    \
    ../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt --model "gpt-4.1-nano" | tee -a $filename && \
    \
    git restore . && \
    clear && \
    cd .. && \
    python buggy_prepare.py > buggy-java-jml-eiffel/classes.txt && \
    cd buggy-java-jml-eiffel && \
    \
    ../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt --model claude-sonnet-4-5 | tee -a $filename
