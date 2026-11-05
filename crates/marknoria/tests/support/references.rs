// Public synthetic source shared by the core audit and real Web-reader corpus.
pub fn raw_html_document() -> String {
    concat!(
        "\u{feff}# 重复 **标题** 😀\r\n\r\n",
        "[heading](#custom-heading) [named](#custom-anchor)\r\n\r\n",
        "![markdown](中文%20pic.png)\r\n\r\n",
        "<div><img src=\"中文%20pic.png\" alt=\"block\" onerror=\"window.bad=1\" srcset=\"https://example.invalid/never.png 2x\"><img src=\"data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII=\" alt=\"inline data\"></div>\r\n\r\n",
        "Inline <img src=\"中文%20pic.png\" alt=\"inline\"> image and <a href=\"next.md\" onclick=\"window.bad=1\">local link</a>.\r\n\r\n",
        "| HTML image | Value |\r\n|---|---|\r\n| <img src=\"中文%20pic.png\" alt=\"cell\"> | safe |\r\n\r\n",
        "<a href=\"javascript:window.bad=1\">unsafe target</a>\r\n\r\n",
        "<div><img src=\"not-accepted.png\"><script>window.bad=1</script></div>\r\n\r\n",
        "<h2 id=\"custom-heading\">Explicit heading</h2>\r\n\r\n",
        "<a id=\"custom-anchor\"></a>Named anchor.\r\n\r\n",
        "<span id=\"source-view\" data-image-reference=\"999\" data-document-anchor=\"forged\">No host identity</span>\r\n\r\n",
        "## 重复 **标题** 😀\r\n\r\n",
        "## 重复 **标题** 😀\r\n\r\n",
        "<img src=\"/中文%20pic.png\" alt=\"root relative\"><img src=\"images\\picture.png\" alt=\"backslash\">\r\n"
    ).into()
}

pub fn source_image() -> image::RgbaImage {
    let mut image=image::RgbaImage::from_fn(4096,1024,|x,y|{
        let value=if y<90 || x%2==1 {255} else {0};image::Rgba([value,value,value,255])
    });
    // Six 5x7 glyphs, one source pixel per stroke: SOURCE.
    let letters:[[u8;7];6]=[
        [14,16,16,14,1,1,30],[14,17,17,17,17,17,14],
        [17,17,17,17,17,17,14],[30,17,17,30,20,18,17],
        [14,17,16,16,16,17,14],[31,16,16,30,16,16,31],
    ];
    for (letter,rows) in letters.iter().enumerate() {
        for (row,bits) in rows.iter().enumerate() {
            for column in 0..5 {if bits&(1<<(4-column))!=0 {image.put_pixel(20+letter as u32*6+column,20+row as u32,image::Rgba([0,0,0,255]));}}
        }
    }
    image
}
