FROM mcr.microsoft.com/devcontainers/rust:1-1-bullseye

# Include lld linker to improve build times either by using environment variable
# RUSTFLAGS="-C link-arg=-fuse-ld=lld" or with Cargo's configuration file (i.e see .cargo/config.toml).
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
   && apt-get -y install clang lld \
   && apt-get autoremove -y && apt-get clean -y

COPY .env_user /tmp/.env
# Source to use it
RUN . /tmp/.env && rm /tmp/.env && \
   if getent passwd $USERNAME; then userdel -f $USERNAME; fi && \
   if getent passwd $USER_UID; then userdel -f $USER_UID; fi && \
   if getent group $GROUPNAME; then groupdel $GROUPNAME; fi && \
   if getent group $USER_GID; then groupdel $USER_GID; fi && \
   groupadd --gid $USER_GID $GROUPNAME && \
   useradd --uid $USER_UID --gid $USER_GID -m $USERNAME -s /bin/bash && \
   mkdir -p /etc/sudoers.d && \
   echo $USERNAME ALL=\(root\) NOPASSWD:ALL >/etc/sudoers.d/$USERNAME && \
   chmod 0440 /etc/sudoers.d/$USERNAME && \
   chown $USERNAME:$GROUPNAME $(getent passwd $USERNAME | cut -d: -f6) && \
   chmod 755 $(getent passwd $USERNAME | cut -d: -f6)


RUN if [ -z "$HTTP_PROXY" ] ; then \
   echo "Bypass proxy configuration."; \
   else \
   echo "Use proxy configuration ($HTTP_PROXY)"; \
   git config --global https.proxy ${HTTP_PROXY}; \
   git config --global http.proxy ${HTTP_PROXY}; \
   echo "httpProxy: \"$HTTP_PROXY\"\nhttpsProxy: \"$HTTP_PROXY\"\n" | sudo tee -a /.yarnrc.yml; \ 
   echo "[http]\nproxy = \"$HTTP_PROXY\"" > ${CARGO_HOME}/config.toml; \
   fi
