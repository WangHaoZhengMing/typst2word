// Test 02_linebreaking/02_cjk_kinsoku_shori.typ
// Covers: CJK Kinsoku Shori (prohibited line-start and line-end punctuation),
// consecutive punctuation compression/overflow, and CJK character grid wrapping.

#set page(
  paper: "a4",
  margin: (top: 2.54cm, bottom: 2.54cm, left: 3.18cm, right: 3.18cm),
)
#set text(font: "Songti SC", size: 11pt, lang: "zh")
#set par(
  justify: true,
  linebreaks: "simple",
  justification-limits: (spacing: (min: 100%, max: 500%)),
  leading: 0.65em,
  spacing: 1.2em,
)

= 1. 中文避头尾法则（Kinsoku Shori）基础测试

在中文排版规范中，句号、逗号、顿号、分号、冒号、问号、感叹号以及右括号、右引号、右书名号等标点符号绝对不允许出现在一行的行首位置。当一行正好填满三十七个汉字（宽度为四百零七点零磅，距离右侧页边距四百一十五点三磅仅剩八点三磅，不足以容纳下一个十一点零磅的全角字符）且紧接着一个全角句号或逗号时，排版引擎必须触发避头尾回溯逻辑。

例如下面这一句特意构造在第三十七个字之后出现全角逗号和句号：测试一二三四五六七八九十甲乙丙丁戊己庚辛壬癸子丑寅卯辰巳午未申酉戌亥天地玄，黄宇宙洪荒日月盈昃辰宿列张寒来暑往秋收冬藏闰余成岁律吕调阳。云腾致雨露结为霜金生丽水玉出昆冈剑号巨阙珠称夜光果珍李柰菜重芥姜。

= 2. 行尾禁则与前引号、左括号、左书名号绑定

与行首禁则相对，左括号（、左书名号《、前双引号“、前单引号‘等开标点符号绝对不允许孤立留在行尾。如果开标点正好落在某一行最后一个全角字位上，它必须连同后面的汉字一起推入下一行行首：天地玄黄宇宙洪荒日月盈昃辰宿列张寒来暑往秋收冬藏闰余成岁律吕调阳云腾致雨《千字文》与“中文排版法则”测试（包含多层括号与书名号的边界情况）。

= 3. 破折号与省略号的双字符不可分割性

中文破折号“——”由两个连续的 `U+2014` 组成，省略号“……”由两个连续的 `U+2026` 组成。在 PTLS7 的 22×22 断行矩阵中，相同类别的破折号之间禁止断开，但破折号整体结束之后可以直接断行：问题还不止这些，香港剩下的最后那条就业通道——也就是外资投行与跨国金融机构的毕业生招聘名额……在过去三年里同样经历了剧烈收缩。
