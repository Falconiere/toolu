FROM oven/bun:1.4.2

RUN apt-get update -qq
RUN apt-get install -y -qq git jq
