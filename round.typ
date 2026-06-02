= with chatgpt 
我会按“Typst FrameItem → Word OOXML”的转换路径讲。重点不是把 PDF 还原成 Word，而是保留可编辑性时该怎么降级。

思考時間: 4s

做 Typst → Word，不建议只 Frame。正确架构是“双通道”：

Typst Source / Content Tree  →  生成可编辑 Word 结构
PagedDocument / Frame        →  补充位置、尺寸、视觉兜底

Typst 官方编译流程是：源码先解析、求值成内容树，再布局成 PagedDocument，其中每页一个 fixed-position frame，最后导出 PDF/PNG/SVG/HTML。也就是说 Frame 已经是“排版后的页面结果”，不是语义文档结构。

你要这样处理：

TextItem
优先合并成 Word 的段落、run、标题、列表项。不要一个 glyph 一个 run。要按同一行、同一字体、同一字号、同一颜色、同一 baseline 合并。Word 对应：

TextItem → w:p / w:r / w:t

难点是：Frame 里文本已经被拆成绝对位置，所以你要反推段落。更好的做法是从 Typst 的 Content Tree 识别 paragraph、heading、list、table，再用 Frame 校准样式。

Shape
简单形状可以转 OOXML DrawingML：

rect / line / ellipse → Word shape

复杂路径、Cetz 图、装饰性图形，直接转 SVG/PNG 插入 Word：

复杂 Shape → SVG/PNG image

这样最稳。否则你会陷入 DrawingML 地狱。

Image
直接映射为 Word 图片：

ImageItem → word/media/* + wp:inline 或 wp:anchor

普通文档用 inline，海报/复杂页面用 anchor 绝对定位。

Group
Group 不能简单展开。它可能包含 transform、clip、opacity、嵌套坐标系。处理策略：

无 transform 的 Group → 递归展开
有 rotate/scale/clip/opacity 的 Group → 整组渲染为 SVG/PNG

这是关键。Word 对复杂 transform 支持很差，硬转会崩。

Link
映射为 Word hyperlink：

Link → w:hyperlink

如果 Link 只是一个透明点击区域，要把它绑定到下面的文字或图片；绑定不了就忽略或生成不可见超链接区域。

Tag
不要当成视觉元素。它更像语义/查询锚点。可用于恢复：

figure 编号
label
cross-reference
outline
bookmark

Word 对应：

Tag / label → bookmarkStart / bookmarkEnd
引用 → field code / hyperlink

核心降级规则可以这么定：

能语义化的 → Word 原生结构
不能语义化但能画的 → DrawingML
复杂视觉对象 → SVG/PNG
无法表达的元信息 → bookmark/customXml/comment

具体映射表：

Typst heading      → Word Heading 1/2/3
paragraph          → w:p
strong/emph        → bold/italic run
enum/list          → numbering.xml
table              → w:tbl
math               → OMML，或 MathML→OMML，失败则 SVG
figure             → image/table + caption
code block         → monospace paragraph/table
page break         → w:br type="page"
columns            → section properties，复杂则降级
absolute layout    → wp:anchor
Cetz/complex frame → SVG/PNG


PagedDocument 比单纯的 Frame 丰富。

关键结构大概是：

PagedDocument {
    pages: Vec<Page>,          // 每页的 frame
    info: DocumentInfo,        // PDF metadata
    introspector: Introspector // 查询 heading/figure/label 等语义信息
}

官方文档也写得很直白：PagedDocument 是“finished document with metadata and page frames”。它不只是画布。

但要分清楚：

Frame/PageFrame 主要是视觉层
PagedDocument.info 是文档元数据
PagedDocument.introspector 是语义/查询层

PDF 里的标题、作者、描述、关键词等，来自：

#set document(
  title: [Hello],
  author: ("A", "B"),
)

这些不是 FrameItem 里的东西，而是 DocumentInfo / PDF metadata。Typst 文档也说明这些 metadata 不渲染在页面里，而是嵌入 PDF。

标题目录、heading、figure、label 这类信息，更多来自 Typst 的 model/introspection 系统。比如 heading 本身有 level、numbering、outlined 等语义属性，Typst 可以自动根据 heading 生成 outline。

所以你做 Typst → Word 时，不应该只看 FrameItem，而应该这样分层：

PagedDocument.info
→ Word core.xml / app.xml 元数据

Introspector / query heading
→ Word Heading 样式、目录、bookmark、cross-reference

Page.frame
→ 视觉布局、图片位置、复杂图形 fallback

FrameItem::Text
→ run / paragraph 的视觉校准

FrameItem::Shape / Group
→ DrawingML 或 SVG/PNG fallback

一句话：FramePage 确实有丰富视觉数据，但“标题等语义信息”主要不在 Frame 本身，而在 PagedDocument.info 和 introspector。你要做高质量 Word，必须同时吃这三层。