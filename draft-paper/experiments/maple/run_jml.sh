cd buggy-java-jml-eiffel/ &&
    clear && \
    cargo build --release -p llm-correct-features && \
    \
    git restore . && \
    clear && \
    cd .. && \
    python buggy_prepare.py | sed -n "1,10p" > buggy-java-jml-eiffel/classes.txt && \
    cd buggy-java-jml-eiffel && \
    \
    ../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt | tee sonnet.jsonl && \
    \
    git restore . && \
    clear && \
    cd .. && \
    python buggy_prepare.py | sed -n "1,10p" > buggy-java-jml-eiffel/classes.txt && \
    cd buggy-java-jml-eiffel && \
    \
    ../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt --model gpt-4o-mini | tee gpt4.jsonl
