from tieba_sidecar.extractor import (
    extract_media_urls,
    mask_nickname,
    parse_comments,
    parse_creator_portrait,
    parse_note_detail,
    parse_post_id,
    parse_search_results,
    parse_sub_comments,
)


def test_search_results_mask_author_and_keep_public_post_fields():
    result = parse_search_results({
        "data": {"card_list": [{
            "cardInfo": "thread",
            "data": {
                "tid": "10559655942", "title": "标题", "content": "正文", "time": 1773552643,
                "post_num": 9, "forum_name": "诸城", "user": {"id": 123456, "show_nickname": "真实昵称"},
            },
        }]},
    }, "测试词")

    assert len(result) == 1
    assert result[0]["id"] == "10559655942"
    assert result[0]["forum"] == "诸城吧"
    assert result[0]["author"] == "真***称"
    assert result[0]["keyword"] == "测试词"
    assert "123456" not in str(result[0])


def test_note_and_comment_extractors_do_not_store_raw_author_ids():
    api = {
        "thread": {"id": 1045, "title": "帖子", "reply_num": 12, "create_time": 1773552643},
        "forum": {"id": 123, "name": "测试"},
        "page": {"total_page": 1},
        "first_floor": {"author_id": 7, "content": [{"type": 0, "text": "正文"}]},
        "post_list": [{"id": 88, "author_id": 9, "time": 1773552643, "content": [{"type": 0, "text": "回复"}], "sub_post_number": 2}],
        "user_list": [{"id": 7, "name_show": "楼主用户"}, {"id": 9, "name_show": "回帖用户"}],
    }
    post = parse_note_detail(api, "主题")
    comments = parse_comments(api, post)

    assert post["id"] == "1045"
    assert post["author"] == "楼***户"
    assert comments[0]["id"] == "88"
    assert comments[0]["author"] == "回***户"
    assert comments[0]["nested"] == 2
    assert "author_id" not in str(post)
    assert "author_id" not in str(comments)


def test_media_extraction_only_reads_content_and_skips_avatar_and_emoticon_fields():
    urls = extract_media_urls([
        {"type": 0, "text": "文字 https://tiebapic.baidu.com/forum/pic/item/sample.jpg"},
        {"type": 3, "cdn_src": "https://imgsrc.baidu.com/forum/pic/item/photo.png"},
        {"avatar": "https://imgsrc.baidu.com/avatar.jpg", "emoji": "https://tb2.bdstatic.com/face.png"},
        {"video_url": "https://tbvideo.bdstatic.com/media/demo.mp4"},
    ])

    assert {item["kind"] for item in urls} == {"图片", "视频"}
    assert len(urls) == 3
    assert all("avatar" not in item["url"] and "face.png" not in item["url"] for item in urls)


def test_creator_and_post_identifiers_are_normalized():
    assert parse_creator_portrait("https://tieba.baidu.com/home/main/?id=tb.1.example&fr=frs") == "tb.1.example"
    assert parse_post_id("https://tieba.baidu.com/p/10451142633?see_lz=1") == "10451142633"
    assert parse_post_id("not-a-post") == ""
    assert mask_nickname("李") == "*"


def test_subcomment_parser_masks_nickname_and_associates_parent():
    page = '''<ul><li class="lzl_single_post j_lzl_s_p" data-field='{"spid":"sp-1","showname":"回复用户"}'>
      <span class="lzl_content_main">楼中楼内容</span><span class="lzl_time">刚刚</span></li></ul>'''
    post = {"id": "p1", "url": "https://tieba.baidu.com/p/p1"}
    parent = {"id": "c1", "forumId": "f1"}

    comments = parse_sub_comments(page, post, parent)

    assert len(comments) == 1
    assert comments[0]["id"] == "sp-1"
    assert comments[0]["parentId"] == "c1"
    assert comments[0]["postId"] == "p1"
    assert comments[0]["author"] == "回***户"
