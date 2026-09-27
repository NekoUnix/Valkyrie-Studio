#include <glad/gl.h>
#include <GLFW/glfw3.h>
#include <imgui.h>
#include <imgui_impl_glfw.h>
#include <imgui_impl_opengl3.h>
#include <nlohmann/json.hpp>
#include "bridge.h"
#include <algorithm>
#include <string>
#include <cmath>
#include <vector>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <cstdlib>
#define STB_IMAGE_WRITE_IMPLEMENTATION
#include <stb_image_write.h>
using json=nlohmann::json;
namespace {
GLFWwindow* window=nullptr;
ImVec2 canvas_pos,canvas_size;
std::string result;
char model_path[2048]="",audio_path[2048]="",background_path[2048]="",output_path[2048]="output/session.webm";
char speech[4096]="Hello! Welcome to my studio.";
int custom_w=1080,custom_h=1920,codec=3,provider=0,selected=-1;
int record_fps=30;
float zoom=1,pan[2]={0,0},bg[4]={0.06f,0.07f,0.11f,0};
char phone_ip[256]="",listen_bind[128]="0.0.0.0",vts_url[1024]="ws://127.0.0.1:8001",camera_source[1024]="0";
int phone_protocol=0,phone_port=49983,listen_port=49983,camera_port=15483,camera_backend=0,camera_w=640,camera_h=480,camera_fps=30;
bool camera_preview=false,settings_loaded=false;
ImGuiStyle base_style;
float applied_scale=0,applied_density=0;
bool audio_panel=false;
char openai_key[1024]="",elevenlabs_key[1024]="",voice_direction[2048]="Speak naturally, warmly and clearly.";
std::string speech_voice="coral",speech_model="gpt-4o-mini-tts";
std::string elevenlabs_voice;
float speech_speed=1;
bool remember_key=true,elevenlabs_remember_key=true,autoplay=true,elevenlabs_autoplay=true,voice_loaded=false,elevenlabs_loaded=false,elevenlabs_auto_selected=false;
const char* protocols[]={"ifacial","udp","vts"};
const char* backends[]={"auto","dshow","msmf","v4l2","avfoundation"};
void caption(const char* text) { ImGui::TextColored(ImVec4(0.37f,0.82f,0.73f,1),"%s",text); }
void help(const char* text) { ImGui::TextWrapped("%s",text); }
void apply_scale(float scale,float density) {
    if(std::abs(scale-applied_scale)<0.01f && std::abs(density-applied_density)<0.01f)return;
    auto& io=ImGui::GetIO();
    ImGui_ImplOpenGL3_DestroyFontsTexture();io.Fonts->Clear();
    ImFontConfig cfg;cfg.SizePixels=std::round(16*scale*density);cfg.OversampleH=2;cfg.OversampleV=2;
    std::string font;
#ifdef _WIN32
    if(auto* root=std::getenv("WINDIR"))font=std::string(root)+"/Fonts/segoeui.ttf";
#elif defined(__APPLE__)
    font="/System/Library/Fonts/Supplemental/Arial.ttf";
#else
    font="/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";
#endif
    if(!font.empty() && std::filesystem::exists(font))io.Fonts->AddFontFromFileTTF(font.c_str(),cfg.SizePixels,&cfg);
    else io.Fonts->AddFontDefault(&cfg);
    io.FontGlobalScale=1/density;
    ImGui::GetStyle()=base_style;ImGui::GetStyle().ScaleAllSizes(scale);
    ImGui_ImplOpenGL3_CreateFontsTexture();applied_scale=scale;applied_density=density;
}
}
int l2d_ui_init(void* pointer) {
    window=static_cast<GLFWwindow*>(pointer);
    IMGUI_CHECKVERSION();ImGui::CreateContext(); ImGui::GetIO().IniFilename=nullptr;
    ImGui::StyleColorsDark(); auto& style=ImGui::GetStyle();
    style.WindowRounding=7;style.FrameRounding=5;style.GrabRounding=4;style.ItemSpacing={10,9};style.WindowPadding={18,18};
    style.Colors[ImGuiCol_WindowBg]=ImVec4(0.055f,0.064f,0.084f,1);
    style.Colors[ImGuiCol_Button]=ImVec4(0.12f,0.28f,0.28f,1);
    style.Colors[ImGuiCol_ButtonHovered]=ImVec4(0.18f,0.44f,0.4f,1);
    style.Colors[ImGuiCol_FrameBg]=ImVec4(0.10f,0.13f,0.17f,1);
    base_style=style;settings_loaded=voice_loaded=elevenlabs_loaded=elevenlabs_auto_selected=false;applied_scale=applied_density=0;
    glfwSetWindowSizeLimits(window,800,600,GLFW_DONT_CARE,GLFW_DONT_CARE);
    return ImGui_ImplGlfw_InitForOpenGL(window,true)&&ImGui_ImplOpenGL3_Init("#version 330 core");
}
void l2d_ui_shutdown() { if(window){ImGui_ImplOpenGL3_Shutdown();ImGui_ImplGlfw_Shutdown();ImGui::DestroyContext();window=nullptr;} }
const char* l2d_ui_frame(unsigned texture,int width,int height,const char* state) {
    try {
    auto status=json::parse(state);json commands=json::array();
    auto guide=status.value("guides",json::object());
    auto settings=status.value("connection_settings",json::object());
    auto voice_status=status.value("voice",json::object());
    auto elevenlabs_status=status.value("elevenlabs",json::object());
    if(!voice_loaded) {
        speech_voice=voice_status.value("voice","coral");speech_model=voice_status.value("model","gpt-4o-mini-tts");
        speech_speed=voice_status.value("speed",1.0f);autoplay=voice_status.value("autoplay",true);
        remember_key=voice_status.value("persistent_storage",false);
        std::snprintf(voice_direction,sizeof(voice_direction),"%s",voice_status.value("instructions","").c_str());voice_loaded=true;
    }
    if(!elevenlabs_loaded) {
        elevenlabs_voice=elevenlabs_status.value("voice_id","");
        elevenlabs_autoplay=elevenlabs_status.value("autoplay",true);
        elevenlabs_remember_key=elevenlabs_status.value("persistent_storage",false);
        elevenlabs_loaded=true;
    }
    if(!elevenlabs_auto_selected && elevenlabs_status.value("configured",false)) {
        provider=1;elevenlabs_auto_selected=true;
    }
    auto ui_settings=status.value("ui_settings",json::object());
    float content_x=1,content_y=1;glfwGetWindowContentScale(window,&content_x,&content_y);
    int pixel_w,pixel_h,window_w,window_h;glfwGetFramebufferSize(window,&pixel_w,&pixel_h);glfwGetWindowSize(window,&window_w,&window_h);
    float density=window_w>0?std::max(1.0f,float(pixel_w)/window_w):1.0f;
    // Retina already scales framebuffer pixels. Windows DPI scales logical widgets.
    float display_scale=std::max(1.0f,content_x/density);
    float s=std::clamp(ui_settings.value("scale",1.0f)*(ui_settings.value("follow_dpi",true)?display_scale:1.0f),0.75f,4.0f);
    apply_scale(s,density);
    static std::string previous_tab_request;
    std::string requested_tab;
    if(status.contains("ui_tab_request") && status["ui_tab_request"].is_object()) {
        auto request=status["ui_tab_request"];
        if(request.value("id","")!=previous_tab_request) {
            requested_tab=request.value("tab","");previous_tab_request=request.value("id","");
        }
    }
    auto tab_flags=[&](const char* name){return requested_tab==name?ImGuiTabItemFlags_SetSelected:ImGuiTabItemFlags_None;};
    if(requested_tab=="Audio")audio_panel=true;
    else if(!requested_tab.empty())audio_panel=false;
    if(!settings_loaded) {
        auto copy=[&](const char* key,char* dest,size_t n){auto value=settings.value(key,"");std::snprintf(dest,n,"%s",value.c_str());};
        copy("phone_ip",phone_ip,sizeof(phone_ip));copy("listen_bind",listen_bind,sizeof(listen_bind));
        copy("vts_url",vts_url,sizeof(vts_url));copy("webcam_source",camera_source,sizeof(camera_source));
        phone_port=settings.value("phone_port",49983);listen_port=settings.value("listen_port",49983);camera_port=settings.value("webcam_port",15483);
        camera_w=settings.value("webcam_width",640);camera_h=settings.value("webcam_height",480);camera_fps=settings.value("webcam_fps",30);
        camera_preview=settings.value("webcam_preview",false);
        for(int i=0;i<3;++i)if(settings.value("phone_protocol","")==protocols[i])phone_protocol=i;
        for(int i=0;i<5;++i)if(settings.value("webcam_backend","")==backends[i])camera_backend=i;
        settings_loaded=true;
        record_fps=int(status.value("record_fps",30.0));
    }
    ImGui_ImplOpenGL3_NewFrame();ImGui_ImplGlfw_NewFrame();ImGui::NewFrame();
    auto size=ImGui::GetIO().DisplaySize;
    const bool compact=size.x<980*s;
    const float left=std::min(325*s,size.x*0.68f),right=compact?0:350*s,top=80*s;
    auto flags=ImGuiWindowFlags_NoTitleBar|ImGuiWindowFlags_NoResize|ImGuiWindowFlags_NoCollapse|ImGuiWindowFlags_NoMove|ImGuiWindowFlags_NoSavedSettings;
    ImGui::SetNextWindowPos({0,0});ImGui::SetNextWindowSize({size.x,top});
    ImGui::Begin("Header",nullptr,flags);caption("VALKYRIE STUDIO");
    ImGui::SameLine(std::max(240*s,size.x-200*s));
    static float edit_percent=100;static bool edit_dpi=true;
    if(ImGui::Button("Settings")) {edit_percent=ui_settings.value("scale",1.0f)*100;edit_dpi=ui_settings.value("follow_dpi",true);ImGui::OpenPopup("Display settings");}
    ImGui::SetNextWindowSize({std::min(470*s,size.x-30),0},ImGuiCond_Appearing);
    ImGui::SetNextWindowPos({size.x/2,size.y/2},ImGuiCond_Appearing,{0.5f,0.5f});
    if(ImGui::BeginPopupModal("Display settings",nullptr,ImGuiWindowFlags_AlwaysAutoResize)) {
        ImGui::TextUnformatted("Interface scale");ImGui::SetNextItemWidth(-1);ImGui::SliderFloat("##scale",&edit_percent,75,250,"%.0f%%");
        ImGui::Checkbox("Follow display DPI",&edit_dpi);
        ImGui::Text("Display: %.0f%% | Effective size: %.0f%%",display_scale*100,s*100);
        help("Scales text, controls and panels. Larger sizes use tabs to keep the canvas visible. Saved for the next launch.");
        if(ImGui::Button("Apply")) {commands.push_back({{"op","ui_settings"},{"scale",edit_percent/100},{"follow_dpi",edit_dpi}});ImGui::CloseCurrentPopup();}
        ImGui::SameLine();if(ImGui::Button("Reset")) {commands.push_back({{"op","ui_settings"},{"scale",1.0},{"follow_dpi",true}});ImGui::CloseCurrentPopup();}
        ImGui::SameLine();if(ImGui::Button("Cancel"))ImGui::CloseCurrentPopup();
        ImGui::EndPopup();
    }
    ImGui::SameLine();
    if(ImGui::Button("A-"))commands.push_back({{"op","ui_settings"},{"scale",std::max(0.75f,ui_settings.value("scale",1.0f)-0.25f)}});
    ImGui::SameLine();
    if(ImGui::Button("A+"))commands.push_back({{"op","ui_settings"},{"scale",std::min(2.5f,ui_settings.value("scale",1.0f)+0.25f)}});
    if(compact) {
        if(ImGui::Button("Controls"))audio_panel=false;ImGui::SameLine();
        if(ImGui::Button("Audio / Export"))audio_panel=true;ImGui::SameLine();
    }
    ImGui::Text("%.1f fps | %d x %d",status.value("fps",0.0),width,height);
    if(status.value("recording",false)){ImGui::SameLine();ImGui::TextColored({1,0.35f,0.38f,1},"REC");}ImGui::End();
    if(!compact || !audio_panel) {
    ImGui::SetNextWindowPos({0,top});ImGui::SetNextWindowSize({left,size.y-top});ImGui::Begin("Controls",nullptr,flags);
    if(ImGui::BeginTabBar("Workspace controls")) {
    if(ImGui::BeginTabItem("Scene",nullptr,tab_flags("Scene"))) {
    caption("SCENE");help("Drop a model, PNG prop, or audio file onto the canvas.");
    ImGui::SetNextItemWidth(-1);ImGui::InputTextWithHint("##model","Path to .model3.json",model_path,sizeof(model_path));
    if(ImGui::Button("Load primary",{-1,0}))commands.push_back({{"op","load_model"},{"path",model_path},{"primary",true}});
    if(ImGui::Button("Add accessory",{-1,0}))commands.push_back({{"op","load_model"},{"path",model_path},{"primary",false}});
    ImGui::Separator();caption("CANVAS");
    if(ImGui::Button("9:16"))commands.push_back({{"op","canvas"},{"width",1080},{"height",1920}});ImGui::SameLine();
    if(ImGui::Button("16:9"))commands.push_back({{"op","canvas"},{"width",1920},{"height",1080}});ImGui::SameLine();
    if(ImGui::Button("1:1"))commands.push_back({{"op","canvas"},{"width",1080},{"height",1080}});
    ImGui::SetNextItemWidth(100*s);ImGui::InputInt("W",&custom_w);ImGui::SameLine();ImGui::SetNextItemWidth(100*s);ImGui::InputInt("H",&custom_h);
    if(ImGui::Button("Apply dimensions",{-1,0}))commands.push_back({{"op","canvas"},{"width",custom_w},{"height",custom_h}});
    if(ImGui::SliderFloat("Zoom",&zoom,0.1f,12.0f))commands.push_back({{"op","view"},{"zoom",zoom},{"x",pan[0]},{"y",pan[1]}});
    if(ImGui::SliderFloat2("Pan",pan,-3,3))commands.push_back({{"op","view"},{"zoom",zoom},{"x",pan[0]},{"y",pan[1]}});
    ImGui::EndTabItem();
    }
    if(ImGui::BeginTabItem("Inputs",nullptr,tab_flags("Inputs"))) {
    caption("PERFORMANCE INPUT");
    for(const char* mode:{"agent","phone","webcam","idle"}) {if(ImGui::RadioButton(mode,status.value("mode","")==mode))commands.push_back({{"op","mode"},{"mode",mode}});}
    ImGui::Separator();
    if(ImGui::CollapsingHeader("Connect an AI agent",ImGuiTreeNodeFlags_DefaultOpen)) {
        auto agent=status.value("agent",json::object());
        help("1. Load a .model3.json in Scene.");
        help("2. Choose agent above so it controls the model.");
        help("3. In this project folder, run: ruby bin/agent --check");
        help("4. Give your AI agent docs/CONNECT_AGENT.md. It sends JSON commands to 127.0.0.1:4141 using the local session token. Never share that token publicly.");
        help("For a long narrated MP4, give the agent docs/AGENT_PERFORMANCE.md. It can write voice lines and head cues, then run scripts/perform.ps1 for synchronized rendering.");
        if(ImGui::Button("Enable agent control"))commands.push_back({{"op","mode"},{"mode","agent"}});
        ImGui::TextColored(agent.value("ready",false)?ImVec4(0.37f,0.82f,0.73f,1):ImVec4(1,0.72f,0.42f,1),
            "%s | %d control commands",agent.value("ready",false)?"READY":"Load a model and enable agent",agent.value("commands",0));
        if(agent.contains("last_control_age") && !agent["last_control_age"].is_null())
            ImGui::Text("Last command: %.1fs ago",agent["last_control_age"].get<float>());
    }
    auto connection_values=[&](){return json{{"phone_protocol",protocols[phone_protocol]},{"phone_ip",phone_ip},{"phone_port",phone_port},
        {"listen_bind",listen_bind},{"listen_port",listen_port},{"vts_url",vts_url},{"webcam_source",camera_source},
        {"webcam_backend",backends[camera_backend]},{"webcam_width",camera_w},{"webcam_height",camera_h},
        {"webcam_fps",camera_fps},{"webcam_port",camera_port},{"webcam_preview",camera_preview}};};
    auto connections=status.value("connections",json::object());
    for(auto& ip:connections.value("addresses",json::array()))ImGui::TextWrapped("PC address: %s",ip.get<std::string>().c_str());
    ImGui::PushItemWidth(std::min(125*s,ImGui::GetContentRegionAvail().x*0.50f));
    if(ImGui::CollapsingHeader("Phone connection",status.value("mode","")=="phone"?ImGuiTreeNodeFlags_DefaultOpen:0)) {
        ImGui::Combo("Protocol",&phone_protocol,"iFacialMocap\0ARKit UDP\0VTube Studio API\0");
        if(phone_protocol==0) {
            ImGui::InputText("Phone IP",phone_ip,sizeof(phone_ip));ImGui::InputInt("Phone port",&phone_port);
        }
        if(phone_protocol==2)ImGui::InputText("VTS URL",vts_url,sizeof(vts_url));
        ImGui::InputText("Listen IP",listen_bind,sizeof(listen_bind));ImGui::InputInt("Listen port",&listen_port);
        help("Use 0.0.0.0 to receive from your phone. In the phone app, set destination to this PC and the listen port.");
        if(ImGui::Button("Connect phone"))commands.push_back({{"op","connection_start"},{"source","phone"},{"settings",connection_values()}});
        ImGui::SameLine();if(ImGui::Button("Stop##phone"))commands.push_back({{"op","connection_stop"},{"source","phone"}});
        auto s=connections.value("phone",json::object());help(s.value("message","Disconnected").c_str());
        if(s.contains("packets"))ImGui::Text("Packets: %d | %.1fs ago",s.value("packets",0),s.value("age",0.0));
        if(phone_protocol==2)help("Enable VTube Studio's API and approve the connection there.");
    }
    if(ImGui::CollapsingHeader("Webcam connection",status.value("mode","")=="webcam"?ImGuiTreeNodeFlags_DefaultOpen:0)) {
        ImGui::InputText("Index / URL",camera_source,sizeof(camera_source));
        help("0 = first camera, 1 = second. IP camera stream URLs are also supported.");
        ImGui::Combo("Backend",&camera_backend,"Automatic\0DirectShow (Win)\0Media Foundation\0V4L2 (Linux)\0AVFoundation (Mac)\0");
        ImGui::InputInt("Camera W",&camera_w);ImGui::InputInt("Camera H",&camera_h);ImGui::InputInt("Camera FPS",&camera_fps);
        ImGui::InputInt("Local port",&camera_port);ImGui::Checkbox("Show camera preview",&camera_preview);
        if(ImGui::Button("Start webcam"))commands.push_back({{"op","connection_start"},{"source","webcam"},{"settings",connection_values()}});
        ImGui::SameLine();if(ImGui::Button("Stop##camera"))commands.push_back({{"op","connection_stop"},{"source","webcam"}});
        auto s=connections.value("webcam",json::object());help(s.value("message","Stopped").c_str());
        if(s.contains("packets"))ImGui::Text("Packets: %d | %.1fs ago",s.value("packets",0),s.value("age",0.0));
    }
    ImGui::PopItemWidth();
    if(ImGui::Button("Recalibrate neutral",{-1,0}))commands.push_back({{"op","calibrate"}});
    ImGui::Text("Neutral samples: %d",status.value("calibration",0));
    for(const char* emotion:{"joy","thinking","angry","surprised","neutral"}) {
        if(ImGui::Button(emotion,{-1,0}))commands.push_back({{"op","emotion"},{"name",emotion},{"duration",3}});
    }
    ImGui::EndTabItem();
    }
    if(ImGui::BeginTabItem("Guides",nullptr,tab_flags("Guides"))) {
        caption("SOCIAL PREVIEW / 9:16");
        bool enabled=guide.value("enabled",true),mock=guide.value("mock_ui",true);
        bool changed=ImGui::Checkbox("Show platform guides",&enabled);
        changed|=ImGui::Checkbox("Show interface mockup",&mock);
        auto name=guide.value("preset","All platforms");
        ImGui::SetNextItemWidth(-1);
        if(ImGui::BeginCombo("##platform",name.c_str())) {
            for(auto& item:guide.value("presets",json::array())) {
                auto label=item.get<std::string>();
                if(ImGui::Selectable(label.c_str(),name==label)) {
                    commands.push_back({{"op","guides"},{"preset",label},{"margins",guide.value("margins",json::array({0.06,0.14,0.20,0.35}))}});
                }
            }
            ImGui::EndCombo();
        }
        float opacity=guide.value("opacity",0.3f);changed|=ImGui::SliderFloat("Shade",&opacity,0,0.8f);
        if(changed)commands.push_back({{"op","guides"},{"enabled",enabled},{"mock_ui",mock},{"opacity",opacity}});
        auto margins=guide.value("margins",json::array({0.06,0.14,0.20,0.35}));
        const char* labels[]={"Left","Top","Right","Bottom"};float values[4];bool adjusted=false;
        for(int i=0;i<4;++i){values[i]=margins[i].get<float>()*100;adjusted|=ImGui::SliderFloat(labels[i],&values[i],0,45,"%.0f%%");}
        if(adjusted)commands.push_back({{"op","guides"},{"preset","Custom"},{"margins",{values[0]/100,values[1]/100,values[2]/100,values[3]/100}}});
        help("Keep faces, logos and captions inside the green area. All platforms uses the most conservative combined margins.");
        help("Preview only: guides never appear in your recording. Estimates vary by phone, captions, ads and app version.");
        if(std::abs(float(width)/height-9.0f/16)>0.01f)help("Choose 9:16 in Scene to display the social overlays.");
        ImGui::EndTabItem();
    }
    ImGui::EndTabBar();
    }
    ImGui::Separator();
    help(status.value("notice","").c_str());ImGui::End();
    }
    if(!compact || audio_panel) {
    ImGui::SetNextWindowPos({compact?0:size.x-right,top});ImGui::SetNextWindowSize({compact?left:right,size.y-top});ImGui::Begin("Inspector",nullptr,flags);
    ImGui::PushItemWidth(std::min(165*s,ImGui::GetContentRegionAvail().x*0.60f));
    caption("AUDIO & SPEECH");
    if(ImGui::CollapsingHeader("Load an audio file")) {
        ImGui::SetNextItemWidth(-1);ImGui::InputTextWithHint("##audio","WAV / MP3 path",audio_path,sizeof(audio_path));
        if(ImGui::Button("Load audio"))commands.push_back({{"op","audio"},{"path",audio_path}});
    }
    ImGui::Combo("Provider",&provider,"OpenAI\0ElevenLabs\0System speech\0");
    if(provider==0) {
        help(voice_status.value("message","Add your OpenAI API key.").c_str());
        if(ImGui::CollapsingHeader("OpenAI voice setup",voice_status.value("configured",false)?0:ImGuiTreeNodeFlags_DefaultOpen)) {
            ImGui::TextUnformatted("API key");ImGui::SetNextItemWidth(-1);
            ImGui::InputTextWithHint("##openai_key","Paste your OpenAI API key",openai_key,sizeof(openai_key),ImGuiInputTextFlags_Password);
            if(voice_status.value("persistent_storage",false))ImGui::Checkbox("Remember on this Windows account",&remember_key);
            else help("Key is kept for this session. OPENAI_API_KEY is also supported.");
            if(ImGui::Button("Save key")) {
                commands.push_back({{"op","voice_configure"},{"api_key",openai_key},{"remember",remember_key}});
                std::memset(openai_key,0,sizeof(openai_key));
            }
            ImGui::SameLine();if(ImGui::Button("Forget key"))commands.push_back({{"op","voice_forget_key"}});
        }
        bool changed=false;
        ImGui::TextUnformatted("Voice");ImGui::SetNextItemWidth(-1);
        if(ImGui::BeginCombo("##voice",speech_voice.c_str())) {
            for(auto& item:voice_status.value("voices",json::array())){auto name=item.get<std::string>();if(ImGui::Selectable(name.c_str(),name==speech_voice)){speech_voice=name;changed=true;}}
            ImGui::EndCombo();
        }
        if(ImGui::CollapsingHeader("Voice direction and model")) {
            ImGui::SetNextItemWidth(-1);
            if(ImGui::BeginCombo("##speech_model",speech_model.c_str())) {
                for(auto& item:voice_status.value("models",json::array())){auto name=item.get<std::string>();if(ImGui::Selectable(name.c_str(),name==speech_model)){speech_model=name;changed=true;}}
                ImGui::EndCombo();
            }
            ImGui::InputTextMultiline("##direction",voice_direction,sizeof(voice_direction),{-1,65*s});
            changed|=ImGui::IsItemDeactivatedAfterEdit();
            changed|=ImGui::SliderFloat("Speed",&speech_speed,0.5f,2.0f,"%.2fx");
        }
        changed|=ImGui::Checkbox("Play generated speech with lip-sync",&autoplay);
        if(changed)commands.push_back({{"op","voice_configure"},{"settings",{{"voice",speech_voice},{"model",speech_model},{"speed",speech_speed},{"instructions",voice_direction},{"autoplay",autoplay}}}});
        help("AI-generated voice. Generating speech uses your OpenAI API account.");
    } else if(provider==1) {
        help(elevenlabs_status.value("message","Add your ElevenLabs API key.").c_str());
        if(ImGui::CollapsingHeader("ElevenLabs connection",elevenlabs_status.value("configured",false)?0:ImGuiTreeNodeFlags_DefaultOpen)) {
            ImGui::TextUnformatted("API key");ImGui::SetNextItemWidth(-1);
            ImGui::InputTextWithHint("##elevenlabs_key","Paste your ElevenLabs API key",elevenlabs_key,sizeof(elevenlabs_key),ImGuiInputTextFlags_Password);
            if(elevenlabs_status.value("persistent_storage",false))ImGui::Checkbox("Remember on this Windows account##elevenlabs",&elevenlabs_remember_key);
            else help("Key is kept for this session. ELEVENLABS_API_KEY is also supported.");
            if(ImGui::Button("Save key & load voices")) {
                commands.push_back({{"op","elevenlabs_configure"},{"api_key",elevenlabs_key},{"remember",elevenlabs_remember_key}});
                std::memset(elevenlabs_key,0,sizeof(elevenlabs_key));
            }
            ImGui::SameLine();if(ImGui::Button("Forget key##elevenlabs"))commands.push_back({{"op","elevenlabs_forget_key"}});
        }
        bool busy=elevenlabs_status.value("busy",false);
        ImGui::BeginDisabled(busy || !elevenlabs_status.value("configured",false));
        if(ImGui::Button(busy?"Loading voices...":"Refresh my voices"))commands.push_back({{"op","elevenlabs_refresh_voices"}});
        ImGui::EndDisabled();
        auto voices=elevenlabs_status.value("voices",json::array());
        std::string current_name="Select a voice";
        for(auto& item:voices)if(item.value("voice_id","")==elevenlabs_voice){current_name=item.value("name","");break;}
        ImGui::TextUnformatted("My voices");ImGui::SetNextItemWidth(-1);
        if(ImGui::BeginCombo("##elevenlabs_voice",current_name.c_str())) {
            for(auto& item:voices) {
                auto id=item.value("voice_id","");auto label=item.value("name","")+"##"+id;
                if(ImGui::Selectable(label.c_str(),id==elevenlabs_voice)) {
                    elevenlabs_voice=id;
                    commands.push_back({{"op","elevenlabs_configure"},{"settings",{{"voice_id",id}}}});
                }
            }
            ImGui::EndCombo();
        }
        if(elevenlabs_voice.empty() && !elevenlabs_status.value("voice_id","").empty())elevenlabs_voice=elevenlabs_status.value("voice_id","");
        if(ImGui::Checkbox("Play generated speech with lip-sync##elevenlabs",&elevenlabs_autoplay))
            commands.push_back({{"op","elevenlabs_configure"},{"settings",{{"autoplay",elevenlabs_autoplay}}}});
        help("Speech generation uses your ElevenLabs account.");
    }
    ImGui::InputTextMultiline("##speech",speech,sizeof(speech),{-1,90*s});
    bool busy=voice_status.value("busy",false);
    ImGui::BeginDisabled(busy || (provider==0 && !voice_status.value("configured",false)) ||
        (provider==1 && (!elevenlabs_status.value("configured",false) || elevenlabs_voice.empty())));
    if(ImGui::Button(busy?"Generating...":"Generate speech",{-1,0})) {
        json request={{"op","tts"},{"provider",std::array<const char*,3>{"openai","elevenlabs","system"}[provider]},{"text",speech}};
        if(provider==0){request["voice"]=speech_voice;request["model"]=speech_model;request["speed"]=speech_speed;request["instructions"]=voice_direction;request["autoplay"]=autoplay;}
        if(provider==1){request["voice"]=elevenlabs_voice;request["autoplay"]=elevenlabs_autoplay;}
        commands.push_back(request);
    }
    if(provider==0 && ImGui::Button("Preview voice",{-1,0}))commands.push_back({{"op","tts"},{"provider","openai"},{"text","Hello! My voice is ready, and I am ready to perform."},
        {"voice",speech_voice},{"model",speech_model},{"speed",speech_speed},{"instructions",voice_direction},{"autoplay",true}});
    if(provider==1 && ImGui::Button("Preview voice##elevenlabs",{-1,0}))commands.push_back({{"op","tts"},{"provider","elevenlabs"},
        {"text","Hello! My voice is ready."},{"voice",elevenlabs_voice},{"autoplay",true}});
    ImGui::EndDisabled();
    if(ImGui::Button("Play loaded audio"))commands.push_back({{"op","audio_play"}});ImGui::SameLine();
    if(ImGui::Button("Stop"))commands.push_back({{"op","audio_stop"}});
    ImGui::Separator();caption("BACKGROUND");
    if(ImGui::ColorEdit4("RGBA",bg))commands.push_back({{"op","background"},{"color",{bg[0],bg[1],bg[2],bg[3]}}});
    ImGui::SetNextItemWidth(-1);ImGui::InputTextWithHint("##bg","PNG / JPG / MP4 path",background_path,sizeof(background_path));
    if(ImGui::Button("Load background"))commands.push_back({{"op","background"},{"path",background_path}});
    ImGui::SameLine();if(ImGui::Button("Clear"))commands.push_back({{"op","background"},{"color",{0,0,0,0}}});
    ImGui::Separator();caption("EXPORT");
    if(ImGui::Combo("Format",&codec,"H.264 MP4\0H.265 MP4\0ProRes 4444\0VP9 alpha\0")) {
        auto path=std::string(output_path);auto dot=path.find_last_of('.');if(dot!=std::string::npos)path.resize(dot);
        path+=(codec==2?".mov":codec==3?".webm":".mp4");std::snprintf(output_path,sizeof(output_path),"%s",path.c_str());
    }
    ImGui::SetNextItemWidth(-1);ImGui::InputText("##output",output_path,sizeof(output_path));
    ImGui::InputInt("Record FPS",&record_fps);
    auto export_state=status.value("export",json());
    auto phase=export_state.is_object()?export_state.value("state",""):"";
    bool saving=phase=="draining"||phase=="saving";
    ImGui::BeginDisabled(saving);
    if(ImGui::Button(status.value("recording",false)?"Finish recording":"Start recording",{-1,0})) {
        if(status.value("recording",false))commands.push_back({{"op","record_stop"}});
        else commands.push_back({{"op","record_start"},{"output",output_path},{"fps",record_fps},{"codec",std::array<const char*,4>{"h264","h265","prores","vp9"}[codec]}});
    }
    ImGui::EndDisabled();
    help("Live: lossless capture. Final compression runs after Stop.");
    if(export_state.is_object()) {
        ImGui::Text("%s | queued: %d",phase.c_str(),export_state.value("queue",0));
        if(saving)ImGui::ProgressBar(export_state.value("progress",0.0f),{-1,0});
        ImGui::Text("Captured: %d | repeated: %d",export_state.value("captured",0),export_state.value("duplicates",0));
        ImGui::Text("Capture misses: %d | queue drops: %d",status.value("missed_capture_ticks",0),export_state.value("dropped",0));
    }
    ImGui::Separator();caption("ATTACHMENTS");
    auto attachments=status.value("attachments",json::array());
    for(size_t i=0;i<attachments.size();++i)if(ImGui::Selectable(attachments[i].value("name","Prop").c_str(),selected==int(i)))selected=int(i);
    if(selected>=0 && selected<int(attachments.size())) {
        auto a=attachments[selected];float scale=a.value("scale",1.0f),rotation=a.value("rotation",0.0f);
        bool changed=ImGui::SliderFloat("Prop scale",&scale,0.01f,4.0f);changed|=ImGui::SliderFloat("Rotation",&rotation,-3.1416f,3.1416f);
        int anchor=a.value("anchor",-1);changed|=ImGui::InputInt("ArtMesh index",&anchor);
        if(changed)commands.push_back({{"op","attachment"},{"index",selected},{"scale",scale},{"rotation",rotation},{"anchor",anchor}});
        if(ImGui::Button("Remove attachment")) {commands.push_back({{"op","attachment_remove"},{"index",selected}});selected=-1;}
    }
    ImGui::Separator();caption("MODEL");help(status.value("model","No model loaded").c_str());
    ImGui::Text("Parameters: %d / ArtMeshes: %d",status.value("parameter_count",0),status.value("drawable_count",0));
    if(ImGui::CollapsingHeader("Parameter controls")) {
        static char filter[128]="";ImGui::SetNextItemWidth(-1);ImGui::InputTextWithHint("##filter","Filter parameter IDs",filter,sizeof(filter));
        if(ImGui::Button("Release manual overrides"))commands.push_back({{"op","parameters_clear"}});
        ImGui::BeginChild("Parameter list",{0,250*s});
        for(auto& parameter:status.value("parameters",json::array())) {
            auto id=parameter["id"].get<std::string>();if(filter[0] && id.find(filter)==std::string::npos)continue;
            float value=parameter["value"],minimum=parameter["min"],maximum=parameter["max"];
            ImGui::TextUnformatted(id.c_str());ImGui::SetNextItemWidth(-1);
            if(ImGui::SliderFloat(("##"+id).c_str(),&value,minimum,maximum))commands.push_back({{"op","parameters"},{"values",{{id,value}}},{"duration",600}});
        }
        ImGui::EndChild();
    }
    ImGui::PopItemWidth();
    ImGui::End();
    }
    ImGui::SetNextWindowPos({left,top});ImGui::SetNextWindowSize({std::max(100.0f,size.x-left-right),size.y-top});ImGui::Begin("Preview",nullptr,flags);
    auto available=ImGui::GetContentRegionAvail();float scale=std::min(available.x/width,available.y/height);
    canvas_size={width*scale,height*scale};auto start=ImGui::GetCursorScreenPos();
    canvas_pos={start.x+(available.x-canvas_size.x)/2,start.y+(available.y-canvas_size.y)/2};
    auto* draw=ImGui::GetWindowDrawList();
    draw->AddRectFilled(canvas_pos,{canvas_pos.x+canvas_size.x,canvas_pos.y+canvas_size.y},IM_COL32(36,39,47,255));
    for(int y=0;y<int(canvas_size.y);y+=20)for(int x=0;x<int(canvas_size.x);x+=20)if((x/20+y/20)%2==0)
        draw->AddRectFilled({canvas_pos.x+x,canvas_pos.y+y},{canvas_pos.x+std::min(float(x+20),canvas_size.x),canvas_pos.y+std::min(float(y+20),canvas_size.y)},IM_COL32(45,49,58,255));
    // Preview shares the premultiplied FBO. Guides never enter exported frames.
    draw->AddCallback([](const ImDrawList*,const ImDrawCmd*){glBlendFuncSeparate(GL_ONE,GL_ONE_MINUS_SRC_ALPHA,GL_ONE,GL_ONE_MINUS_SRC_ALPHA);},nullptr);
    draw->AddImage((ImTextureID)(intptr_t)texture,canvas_pos,{canvas_pos.x+canvas_size.x,canvas_pos.y+canvas_size.y},{0,1},{1,0});
    draw->AddCallback(ImDrawCallback_ResetRenderState,nullptr);
    if(guide.value("enabled",true) && std::abs(float(width)/height-9.0f/16)<0.01f) {
        auto m=guide.value("margins",json::array({0.06,0.14,0.20,0.35}));
        auto point=[&](float x,float y){return ImVec2(canvas_pos.x+x*canvas_size.x,canvas_pos.y+y*canvas_size.y);};
        auto overlay_text=[&](ImVec2 p,ImU32 color,const char* text){draw->AddText(ImGui::GetFont(),std::min(ImGui::GetFontSize(),canvas_size.x*0.04f),p,color,text);};
        float l=m[0],t=m[1],r=1-m[2].get<float>(),b=1-m[3].get<float>();
        auto shade=IM_COL32(22,10,30,int(255*guide.value("opacity",0.3f)));
        draw->PushClipRect(canvas_pos,point(1,1),true);
        draw->AddRectFilled(point(0,0),point(1,t),shade);draw->AddRectFilled(point(0,b),point(1,1),shade);
        draw->AddRectFilled(point(0,t),point(l,b),shade);draw->AddRectFilled(point(r,t),point(1,b),shade);
        draw->AddRect(point(l,t),point(r,b),IM_COL32(107,226,195,240),0,0,1.5f);
        auto title=guide.value("preset","All platforms");
        draw->AddRectFilled(point(0,0),point(1,0.055f),IM_COL32(15,20,30,230));
        overlay_text(point(0.025f,0.015f),IM_COL32(175,246,225,255),title.c_str());
        overlay_text(point(l+0.01f,t+0.01f),IM_COL32(175,246,225,255),"SAFE AREA");
        if(guide.value("mock_ui",true)) {
            auto ink=IM_COL32(235,240,249,195);
            overlay_text(point(0.07f,b+0.035f),ink,"@your_account");
            overlay_text(point(0.07f,b+0.075f),ink,"Caption / description");
            for(int i=0;i<2;++i)draw->AddLine(point(0.07f,b+0.12f+i*0.025f),point(0.62f-i*0.12f,b+0.12f+i*0.025f),ink,2);
            draw->AddRect(point(0.07f,0.925f),point(0.93f,0.973f),ink,4);
            overlay_text(point(0.11f,0.937f),ink,"Navigation / reply / CTA");
            if(m[2].get<float>()>0.1f) {
                for(int i=0;i<4;++i) {
                    float y=0.43f+i*0.095f;draw->AddCircle(point(0.91f,y),canvas_size.x*0.033f,ink,20,1.5f);
                    const char* icons[]={"+","Like","Chat","Share"};
                    overlay_text(point(0.865f,y+0.025f),ink,icons[i]);
                }
            }
        }
        draw->PopClipRect();
    }
    ImGui::End();
    if(!status.value("error","").empty()) {
        ImGui::SetNextWindowPos({left+20*s,size.y-150*s});ImGui::SetNextWindowSize({std::max(100.0f,size.x-left-right-40*s),140*s});
        ImGui::Begin("Status",nullptr,flags);ImGui::TextWrapped("%s",status["error"].get<std::string>().c_str());ImGui::End();
    }
    ImGui::Render();int fw,fh;glfwGetFramebufferSize(window,&fw,&fh);glBindFramebuffer(GL_FRAMEBUFFER,0);glViewport(0,0,fw,fh);
    glClearColor(0.035f,0.043f,0.06f,1);glClear(GL_COLOR_BUFFER_BIT);ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());
    result=commands.dump();return result.c_str();
    }catch(const std::exception&){result="[]";return result.c_str();}
}
int l2d_ui_canvas_point(double x,double y,float* out) {
    if(!out||canvas_size.x<=0||canvas_size.y<=0||x<canvas_pos.x||y<canvas_pos.y||x>canvas_pos.x+canvas_size.x||y>canvas_pos.y+canvas_size.y)return 0;
    out[0]=float((x-canvas_pos.x)/canvas_size.x*2-1);out[1]=float(1-(y-canvas_pos.y)/canvas_size.y*2);return 1;
}
int l2d_ui_snapshot(const char* path) {
    int w,h;glfwGetFramebufferSize(window,&w,&h);
    std::vector<unsigned char> raw(size_t(w)*h*4), upright(raw.size());
    glBindFramebuffer(GL_FRAMEBUFFER,0);glReadBuffer(GL_BACK);glPixelStorei(GL_PACK_ALIGNMENT,1);
    glReadPixels(0,0,w,h,GL_RGBA,GL_UNSIGNED_BYTE,raw.data());
    for(int y=0;y<h;++y)std::copy_n(raw.data()+size_t(h-1-y)*w*4,size_t(w)*4,upright.data()+size_t(y)*w*4);
    return stbi_write_png(path,w,h,4,upright.data(),w*4);
}
