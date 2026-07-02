import { defineConfig } from "astro/config";

export default defineConfig({
	redirects: {
		"/examples":
			"https://github.com/punctuations/fossil/blob/main/examples/README.md#lossy",
		"/install":
			"https://raw.githubusercontent.com/punctuations/fossil/refs/heads/main/install.sh",
		"/man":
			"https://raw.githubusercontent.com/punctuations/fossil/refs/heads/main/share/fossil.1",
		"/bash":
			"https://raw.githubusercontent.com/punctuations/fossil/refs/heads/main/share/fossil.bash",
		"/fish":
			"https://raw.githubusercontent.com/punctuations/fossil/refs/heads/main/share/fossil.fish",
		"/zsh":
			"https://raw.githubusercontent.com/punctuations/fossil/refs/heads/main/share/fossil.zsh",
	},
});
