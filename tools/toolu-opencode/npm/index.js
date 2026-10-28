// `@toolu/opencode` is deprecated: an OpenCode V2 plugin that only reports it. It runs no hooks.

const NOTICE = `@toolu/opencode is deprecated and runs none of toolu's hooks, so toolu's gates are off in this session. Install the native binary and the OpenCode plugins with it:
  curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash   (or: brew install falconiere/tap/toolu)
  toolu plugins install --host opencode
Then remove @toolu/opencode from the plugins of opencode.json. See https://github.com/Falconiere/toolu/blob/main/docs/opencode.md`;

export default {
  id: "toolu.deprecated",
  async setup(ctx) {
    console.warn(NOTICE);
    const told = new Set();
    await ctx.session.hook("prompt", async (event) => {
      if (!event.sessionID || told.has(event.sessionID)) return;
      told.add(event.sessionID);
      try {
        await ctx.session.instructions.entry.put({
          sessionID: event.sessionID,
          key: "toolu.deprecated",
          value: `Tell the user: ${NOTICE}`,
        });
      } catch (error) {
        console.warn(`@toolu/opencode: ${error?.message ?? error}`);
      }
    });
  },
};
