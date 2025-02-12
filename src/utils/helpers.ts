export function stripMarkdown(text: string): string {
	return (
		text
			// Remove headers (#)
			.replace(/#{1,6}\s/g, "")
			// Remove bold/italic
			.replace(/[*_]{1,3}(.*?)[*_]{1,3}/g, "$1")
			// Remove links
			.replace(/\[([^\]]+)\]\([^\)]+\)/g, "$1")
			// Remove images
			.replace(/!\[([^\]]+)\]\([^\)]+\)/g, "$1")
			// Remove blockquotes
			.replace(/^\s*>\s/gm, "")
			// Remove code blocks
			.replace(/```[\s\S]*?```/g, "")
			// Remove inline code
			.replace(/`([^`]+)`/g, "$1")
			// Remove horizontal rules
			.replace(/^\s*[-*_]{3,}\s*$/gm, "")
			// Remove lists
			.replace(/^[\s*-]+(.*)/gm, "$1")
			// Clean up extra whitespace
			.replace(/\n\s*\n/g, "\n")
			.trim()
	);
}

export function truncateText(text: string, maxLength: number = 60): string {
	if (text.length <= maxLength) return text;
	return text.slice(0, maxLength).trim() + "...";
}


