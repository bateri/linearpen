-- What a file is, by its name or the type the terminal guessed, for the words and the icons of the
-- page: the icon of a kind of file, what a card calls it, which a picture the kit draws is. The page
-- never reads a file, so none of this is by the bytes: the program holds that (`sniff`).

local kinds = {}

-- The extension of `name`, lower case, or "".
function kinds.extension(name)
	return string.lower(string.match(name or "", "%.(%w+)$") or "")
end

-- The extension that goes with a media type of a picture, or nil.
local TYPES = {
	["image/png"] = "png",
	["image/jpeg"] = "jpg",
	["image/jpg"] = "jpg",
	["image/gif"] = "gif",
	["image/webp"] = "webp",
}

function kinds.extension_of_type(media)
	return TYPES[string.lower(media or "")]
end

-- The kinds the kit draws as pictures: PNG and JPEG.
local DRAWN = { png = true, jpg = true, jpeg = true }

-- Whether the kit draws the file as a picture, by the type the terminal says or, where it says none, by
-- the name.
function kinds.drawn_by_type(media, name)
	local ext = kinds.extension_of_type(media)
	local kind = string.lower(media or "")
	if ext ~= nil then
		return DRAWN[ext] == true
	end
	if kind == "" or kind == "application/octet-stream" then
		return DRAWN[kinds.extension(name)] == true
	end
	return false
end

-- Whether the name says the file is one the page can draw: a PNG, a JPEG or a PDF.
function kinds.drawable_name(name)
	local ext = kinds.extension(name)
	return DRAWN[ext] == true or ext == "pdf"
end

local ICONS = {
	pdf = "file-type-pdf",
	png = "photo",
	jpg = "photo",
	jpeg = "photo",
	gif = "photo",
	webp = "photo",
	svg = "photo",
	heic = "photo",
	mp4 = "movie",
	mov = "movie",
	webm = "movie",
	mkv = "movie",
	zip = "file-zip",
	gz = "file-zip",
	tgz = "file-zip",
	tar = "file-zip",
	["7z"] = "file-zip",
	rar = "file-zip",
	txt = "file-text",
	log = "file-text",
	md = "file-text",
	csv = "file-text",
	json = "file-text",
}

-- The icon (a file of assets/icons) of a file of `name`, or of `media` where the name says nothing.
function kinds.icon(name, media)
	local ext = kinds.extension(name)
	if ICONS[ext] ~= nil then
		return ICONS[ext]
	end
	local made = kinds.extension_of_type(media)
	if made ~= nil then
		return ICONS[made]
	end
	return "file"
end

-- What a card calls a file: `PDF`, `LOG` (its extension, upper case), else `File`.
function kinds.label(name)
	local ext = kinds.extension(name)
	if ext == "" or #ext > 5 then
		return "File"
	end
	return string.upper(ext)
end

function kinds.size_words(bytes)
	if bytes >= 1048576 then
		return string.format("%.1f MB", bytes / 1048576)
	elseif bytes >= 1024 then
		return string.format("%d KB", math.floor(bytes / 1024 + 0.5))
	end
	return bytes .. " B"
end

return kinds
