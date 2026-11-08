# Linux runner image for the Magpie maintenance loop.
# Build context must contain powershell-7.4.6-linux-x64.tar.gz.
FROM node:24-bookworm

ARG PWSH_TARBALL=powershell-7.4.6-linux-x64.tar.gz
COPY ${PWSH_TARBALL} /tmp/pwsh.tar.gz
RUN mkdir -p /opt/microsoft/powershell/7 \
    && tar -xzf /tmp/pwsh.tar.gz -C /opt/microsoft/powershell/7 \
    && chmod +x /opt/microsoft/powershell/7/pwsh \
    && ln -sf /opt/microsoft/powershell/7/pwsh /usr/local/bin/pwsh \
    && rm -f /tmp/pwsh.tar.gz

RUN npm install -g @opencode/cli@2.2.6 --registry=https://registry.npmmirror.com \
    && npm cache clean --force

WORKDIR /workspace
CMD ["sleep", "infinity"]
