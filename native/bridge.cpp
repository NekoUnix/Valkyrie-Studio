#define GLAD_GL_IMPLEMENTATION
#include <glad/gl.h>
#include <GLFW/glfw3.h>
#define STB_IMAGE_IMPLEMENTATION
#include <stb_image.h>
#include <nlohmann/json.hpp>
#include <PurismCore.h>
#include "bridge.h"
#include <algorithm>
#include <array>
#include <cmath>
#include <cstring>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <map>
#include <memory>
#include <numeric>
#include <stdexcept>
#include <string>
#include <vector>

using json = nlohmann::json;
namespace fs = std::filesystem;
namespace {
thread_local std::string error, response;
struct Canvas { GLuint fbo=0, texture=0; int width=0, height=0; };
std::map<GLuint, Canvas> canvases;
std::map<GLuint, std::array<int, 2>> textures;
GLuint program=0, vao=0, vbo=0, ebo=0;
struct MeshUniforms {
    GLint transform=-1, rotation=-1, opacity=-1, multiply=-1, screen=-1;
    GLint masked=-1, inverted=-1, maskpass=-1, tex=-1, mask_tex=-1;
} mesh_uniforms;
Canvas mask;
Canvas capture_canvas;
GLuint capture_program=0;
struct CaptureSlot { GLuint pbo=0; GLsync fence=nullptr; int64_t frame=0; };
std::array<CaptureSlot,3> capture_slots;
size_t capture_head=0,capture_tail=0,capture_pending=0;
bool initialized=false;

std::vector<unsigned char> read_file(const fs::path& path) {
    std::ifstream stream(path, std::ios::binary | std::ios::ate);
    if (!stream) throw std::runtime_error("Cannot read: " + path.u8string());
    auto size = stream.tellg();
    if (size <= 0 || size > 512LL*1024*1024) throw std::runtime_error("Invalid asset size");
    std::vector<unsigned char> bytes(static_cast<size_t>(size));
    stream.seekg(0); stream.read(reinterpret_cast<char*>(bytes.data()), size);
    if (!stream) throw std::runtime_error("Incomplete asset read");
    return bytes;
}
fs::path asset_path(const fs::path& root, const std::string& relative) {
    auto candidate = fs::weakly_canonical(root / fs::u8path(relative));
    auto rel = fs::relative(candidate, fs::weakly_canonical(root));
    if (rel.empty() || *rel.begin() == ".." || rel.is_absolute()) throw std::runtime_error("Model asset escapes its directory");
    return candidate;
}
GLuint shader(GLenum type, const char* source) {
    GLuint s=glCreateShader(type); glShaderSource(s,1,&source,nullptr); glCompileShader(s);
    GLint ok; glGetShaderiv(s,GL_COMPILE_STATUS,&ok);
    if (!ok) { char log[2048]; glGetShaderInfoLog(s,sizeof(log),nullptr,log); glDeleteShader(s); throw std::runtime_error(log); }
    return s;
}
Canvas make_canvas(int w,int h) {
    GLint max_size; glGetIntegerv(GL_MAX_TEXTURE_SIZE,&max_size);
    if(w<16 || h<16 || w>8192 || h>8192 || w>max_size || h>max_size) throw std::runtime_error("Canvas size exceeds supported limits");
    Canvas c; c.width=w; c.height=h;
    glGenTextures(1,&c.texture); glBindTexture(GL_TEXTURE_2D,c.texture);
    glTexImage2D(GL_TEXTURE_2D,0,GL_RGBA8,w,h,0,GL_RGBA,GL_UNSIGNED_BYTE,nullptr);
    glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_MIN_FILTER,GL_LINEAR); glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_MAG_FILTER,GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_WRAP_S,GL_CLAMP_TO_EDGE); glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_WRAP_T,GL_CLAMP_TO_EDGE);
    glGenFramebuffers(1,&c.fbo); glBindFramebuffer(GL_FRAMEBUFFER,c.fbo);
    glFramebufferTexture2D(GL_FRAMEBUFFER,GL_COLOR_ATTACHMENT0,GL_TEXTURE_2D,c.texture,0);
    if(glCheckFramebufferStatus(GL_FRAMEBUFFER)!=GL_FRAMEBUFFER_COMPLETE) {
        glDeleteFramebuffers(1,&c.fbo); glDeleteTextures(1,&c.texture); throw std::runtime_error("Incomplete RGBA framebuffer");
    }
    return c;
}
void destroy(Canvas& c) { if(c.fbo) glDeleteFramebuffers(1,&c.fbo); if(c.texture) glDeleteTextures(1,&c.texture); c={}; }
Canvas& canvas(GLuint id) {
    auto it=canvases.find(id); if(it==canvases.end()) throw std::runtime_error("Invalid framebuffer handle"); return it->second;
}
void bind(const Canvas& c) {
    glBindFramebuffer(GL_FRAMEBUFFER,c.fbo); glViewport(0,0,c.width,c.height);
    glDisable(GL_DEPTH_TEST); glDisable(GL_STENCIL_TEST); glDisable(GL_SCISSOR_TEST); glDisable(GL_CULL_FACE);
    glColorMask(GL_TRUE,GL_TRUE,GL_TRUE,GL_TRUE);
}
void prepare_mesh(const float* transform) {
    glUseProgram(program); glBindVertexArray(vao);
    glUniform4fv(mesh_uniforms.transform,1,transform);
    glUniform1f(mesh_uniforms.rotation,transform[4]);
    glUniform1i(mesh_uniforms.tex,0); glUniform1i(mesh_uniforms.mask_tex,1);
}
// Premultiplied internal color, straight-alpha readback for FFmpeg.
void mesh(GLuint texture,const std::vector<float>& vertices,const unsigned short* indices,int count,
          float opacity,const float* multiply,const float* screen,int mode,int masked,bool inverted,bool maskpass=false) {
    glBindBuffer(GL_ARRAY_BUFFER,vbo); glBufferData(GL_ARRAY_BUFFER,vertices.size()*sizeof(float),vertices.data(),GL_STREAM_DRAW);
    glBindBuffer(GL_ELEMENT_ARRAY_BUFFER,ebo); glBufferData(GL_ELEMENT_ARRAY_BUFFER,count*sizeof(unsigned short),indices,GL_STREAM_DRAW);
    glEnableVertexAttribArray(0); glVertexAttribPointer(0,2,GL_FLOAT,GL_FALSE,4*sizeof(float),nullptr);
    glEnableVertexAttribArray(1); glVertexAttribPointer(1,2,GL_FLOAT,GL_FALSE,4*sizeof(float),reinterpret_cast<void*>(2*sizeof(float)));
    glUniform1f(mesh_uniforms.opacity,opacity);
    glUniform4fv(mesh_uniforms.multiply,1,multiply); glUniform4fv(mesh_uniforms.screen,1,screen);
    glUniform1i(mesh_uniforms.masked,masked); glUniform1i(mesh_uniforms.inverted,inverted);
    glUniform1i(mesh_uniforms.maskpass,maskpass);
    glActiveTexture(GL_TEXTURE0); glBindTexture(GL_TEXTURE_2D,texture);
    glActiveTexture(GL_TEXTURE1); glBindTexture(GL_TEXTURE_2D,masked?mask.texture:0);
    glEnable(GL_BLEND); glBlendEquation(GL_FUNC_ADD);
    if(mode==1) glBlendFuncSeparate(GL_ONE,GL_ONE,GL_ZERO,GL_ONE);
    else if(mode==2) glBlendFuncSeparate(GL_DST_COLOR,GL_ONE_MINUS_SRC_ALPHA,GL_ZERO,GL_ONE);
    else glBlendFuncSeparate(GL_ONE,GL_ONE_MINUS_SRC_ALPHA,GL_ONE,GL_ONE_MINUS_SRC_ALPHA);
    glDrawElements(GL_TRIANGLES,count,GL_UNSIGNED_SHORT,nullptr);
}
const float white[4]={1,1,1,1}, black[4]={0,0,0,0}, identity[5]={1,1,0,0,0};

struct AlignedBytes {
    std::vector<unsigned char> bytes;
    void* data=nullptr;
    void allocate(size_t size,size_t alignment) {
        bytes.resize(size+alignment-1);
        const auto address=reinterpret_cast<uintptr_t>(bytes.data());
        data=reinterpret_cast<void*>((address+alignment-1)&~(alignment-1));
    }
};
struct Vec2 { float x=0, y=0; };
Vec2 operator+(Vec2 a,Vec2 b) { return {a.x+b.x,a.y+b.y}; }
Vec2 operator-(Vec2 a,Vec2 b) { return {a.x-b.x,a.y-b.y}; }
Vec2 operator*(Vec2 a,float f) { return {a.x*f,a.y*f}; }
struct PhysicsInput { int parameter=-1; float weight=0; int type=0; bool reflect=false; };
struct PhysicsOutput { int parameter=-1, vertex=0, type=0; float scale=0, weight=0; bool reflect=false; };
struct Particle { Vec2 rest, position, previous; float mobility=0, delay=0, acceleration=0, radius=0; };
struct PhysicsSetting {
    std::vector<PhysicsInput> inputs;
    std::vector<PhysicsOutput> outputs;
    std::vector<Particle> particles;
    float accumulator=0;
    float norm_position_min=-10, norm_position_default=0, norm_position_max=10;
    float norm_angle_min=-10, norm_angle_default=0, norm_angle_max=10;
};
struct Model {
    AlignedBytes moc_memory, model_memory;
    csmMoc* moc=nullptr; csmModel* core=nullptr;
    std::vector<GLuint> tex; std::vector<std::string> ids; std::vector<int> anchor_edges;
    std::vector<float> vertices;
    std::vector<int> order, last_render_orders;
    std::vector<PhysicsSetting> physics;
    std::string info;
    ~Model() {
        for(auto t:tex) l2d_texture_destroy(t);
    }
};
std::map<void*, std::unique_ptr<Model>> models;
Model& model(void* p) { auto i=models.find(p); if(i==models.end()) throw std::runtime_error("Invalid model handle"); return *i->second; }
void drawable(Model& m,int index,const float* transform,int masked,bool pass=false) {
    auto* core=m.core;
    auto n=csmGetDrawableVertexCounts(core)[index];
    auto* positions=csmGetDrawableVertexPositions(core)[index]; auto* uv=csmGetDrawableVertexUvs(core)[index];
    m.vertices.resize(n*4);
    for(int j=0;j<n;++j) {
        auto* vertex=m.vertices.data()+j*4;
        vertex[0]=positions[j].X; vertex[1]=positions[j].Y;
        vertex[2]=uv[j].X; vertex[3]=1-uv[j].Y;
    }
    auto mult=csmGetDrawableMultiplyColors(core)[index], scr=csmGetDrawableScreenColors(core)[index];
    float multiply[4]={mult.X,mult.Y,mult.Z,mult.W}, screen[4]={scr.X,scr.Y,scr.Z,scr.W};
    auto texture=csmGetDrawableTextureIndices(core)[index];
    if(texture<0 || texture>=int(m.tex.size())) throw std::runtime_error("Drawable references missing texture");
    auto blend=csmGetDrawableBlendModes(core)[index];
    mesh(m.tex[texture],m.vertices,csmGetDrawableIndices(core)[index],csmGetDrawableIndexCounts(core)[index],
      pass?1.0f:csmGetDrawableOpacities(core)[index],multiply,screen,pass?0:(blend&0xff),masked,
      (csmGetDrawableConstantFlags(core)[index]&csmIsInvertedMask)!=0,pass);
}
}

#define TRY try { error.clear();
#define FAIL_RETURN(value) } catch(const std::exception& e) { error=e.what(); return value; } catch(...) { error="Unknown native error"; return value; }
const char* l2d_last_error() { return error.c_str(); }
int l2d_init() { TRY
    if(initialized) return 1;
    if(!gladLoadGL(reinterpret_cast<GLADloadfunc>(glfwGetProcAddress))) throw std::runtime_error("OpenGL 3.3 load failed");
    GLint major, minor; glGetIntegerv(GL_MAJOR_VERSION,&major); glGetIntegerv(GL_MINOR_VERSION,&minor);
    if(major<3 || (major==3 && minor<3)) throw std::runtime_error("OpenGL 3.3 Core required");
    const char* vertex=R"(#version 330 core
      layout(location=0) in vec2 pos; layout(location=1) in vec2 uv;
      uniform vec4 transform; uniform float rotation; out vec2 texUV; out vec2 maskUV;
      void main(){float c=cos(rotation),s=sin(rotation);vec2 q=vec2(c*pos.x-s*pos.y,s*pos.x+c*pos.y);
      vec2 p=q*transform.xy+transform.zw; gl_Position=vec4(p,0,1); texUV=uv; maskUV=p*0.5+0.5;})";
    const char* fragment=R"(#version 330 core
      in vec2 texUV; in vec2 maskUV; out vec4 color;
      uniform sampler2D tex; uniform sampler2D maskTex; uniform float opacity;
      uniform vec4 multiplyColor,screenColor; uniform bool masked,inverted,maskpass;
      void main(){vec4 c=texture(tex,texUV); c.rgb*=multiplyColor.rgb;
      c.rgb=c.rgb+screenColor.rgb-c.rgb*screenColor.rgb;
      c.a*=opacity; if(masked){float m=texture(maskTex,maskUV).a;c.a*=inverted?1.0-m:m;}
      color=maskpass?vec4(c.a):vec4(c.rgb*c.a,c.a);})";
    GLuint vs=shader(GL_VERTEX_SHADER,vertex), fs=shader(GL_FRAGMENT_SHADER,fragment);
    program=glCreateProgram(); glAttachShader(program,vs); glAttachShader(program,fs); glLinkProgram(program);
    glDeleteShader(vs); glDeleteShader(fs); GLint ok; glGetProgramiv(program,GL_LINK_STATUS,&ok);
    if(!ok) throw std::runtime_error("OpenGL shader link failed");
    mesh_uniforms={
        glGetUniformLocation(program,"transform"),glGetUniformLocation(program,"rotation"),
        glGetUniformLocation(program,"opacity"),glGetUniformLocation(program,"multiplyColor"),
        glGetUniformLocation(program,"screenColor"),glGetUniformLocation(program,"masked"),
        glGetUniformLocation(program,"inverted"),glGetUniformLocation(program,"maskpass"),
        glGetUniformLocation(program,"tex"),glGetUniformLocation(program,"maskTex")};
    glGenVertexArrays(1,&vao); glGenBuffers(1,&vbo); glGenBuffers(1,&ebo);
    initialized=true; return 1;
FAIL_RETURN(0) }
void l2d_capture_reset() {
    for(auto& s:capture_slots) {
        if(s.fence) glDeleteSync(s.fence);
        if(s.pbo) glDeleteBuffers(1,&s.pbo);
        s={};
    }
    capture_head=capture_tail=capture_pending=0;
    destroy(capture_canvas);
}
int l2d_capture_submit(unsigned id,int64_t frame) { TRY
    auto& c=canvas(id);
    if(capture_canvas.width!=c.width || capture_canvas.height!=c.height) {
        l2d_capture_reset(); capture_canvas=make_canvas(c.width,c.height);
        for(auto& s:capture_slots) {
            glGenBuffers(1,&s.pbo); glBindBuffer(GL_PIXEL_PACK_BUFFER,s.pbo);
            glBufferData(GL_PIXEL_PACK_BUFFER,size_t(c.width)*c.height*4,nullptr,GL_STREAM_READ);
        }
        glBindBuffer(GL_PIXEL_PACK_BUFFER,0);
    }
    if(capture_pending==capture_slots.size()) return 0;
    if(!capture_program) {
        GLuint vs=shader(GL_VERTEX_SHADER,R"(#version 330 core
          out vec2 uv; void main(){vec2 p=vec2((gl_VertexID<<1)&2,gl_VertexID&2);
          uv=vec2(p.x,1.0-p.y);gl_Position=vec4(p*2.0-1.0,0,1);})");
        GLuint fs=shader(GL_FRAGMENT_SHADER,R"(#version 330 core
          in vec2 uv; uniform sampler2D tex; out vec4 color;
          void main(){vec4 c=texture(tex,uv);color=vec4(c.a>0.0?clamp(c.rgb/c.a,0.0,1.0):vec3(0),c.a);})");
        capture_program=glCreateProgram();glAttachShader(capture_program,vs);glAttachShader(capture_program,fs);
        glLinkProgram(capture_program);glDeleteShader(vs);glDeleteShader(fs);
        GLint ok;glGetProgramiv(capture_program,GL_LINK_STATUS,&ok);
        if(!ok) throw std::runtime_error("Capture shader link failed");
    }
    // Convert alpha and orientation on the GPU; three reusable PBOs hide transfer latency.
    bind(capture_canvas);glDisable(GL_BLEND);glUseProgram(capture_program);glBindVertexArray(vao);
    glActiveTexture(GL_TEXTURE0);glBindTexture(GL_TEXTURE_2D,c.texture);
    glUniform1i(glGetUniformLocation(capture_program,"tex"),0);glDrawArrays(GL_TRIANGLES,0,3);
    auto& s=capture_slots[capture_head];glBindBuffer(GL_PIXEL_PACK_BUFFER,s.pbo);
    glPixelStorei(GL_PACK_ALIGNMENT,1);glReadPixels(0,0,c.width,c.height,GL_RGBA,GL_UNSIGNED_BYTE,nullptr);
    glBindBuffer(GL_PIXEL_PACK_BUFFER,0);s.fence=glFenceSync(GL_SYNC_GPU_COMMANDS_COMPLETE,0);s.frame=frame;
    glFlush();capture_head=(capture_head+1)%capture_slots.size();++capture_pending;return 1;
FAIL_RETURN(-1) }
int l2d_capture_poll(unsigned char* out,size_t size,int64_t* frame) { TRY
    if(!capture_pending) return 0;
    if(!out || !frame || size!=size_t(capture_canvas.width)*capture_canvas.height*4)
        throw std::runtime_error("Capture buffer size mismatch");
    auto& s=capture_slots[capture_tail];auto state=glClientWaitSync(s.fence,0,0);
    if(state==GL_TIMEOUT_EXPIRED) return 0;
    if(state==GL_WAIT_FAILED) throw std::runtime_error("Capture fence failed");
    glBindBuffer(GL_PIXEL_PACK_BUFFER,s.pbo);
    auto* data=glMapBufferRange(GL_PIXEL_PACK_BUFFER,0,size,GL_MAP_READ_BIT);
    if(!data){glBindBuffer(GL_PIXEL_PACK_BUFFER,0);throw std::runtime_error("Capture map failed");}
    std::memcpy(out,data,size);auto ok=glUnmapBuffer(GL_PIXEL_PACK_BUFFER);glBindBuffer(GL_PIXEL_PACK_BUFFER,0);
    glDeleteSync(s.fence);s.fence=nullptr;*frame=s.frame;
    capture_tail=(capture_tail+1)%capture_slots.size();--capture_pending;
    if(!ok) throw std::runtime_error("Capture buffer corrupted");
    return 1;
FAIL_RETURN(-1) }
void l2d_shutdown() {
    if(!initialized) return;
    l2d_capture_reset();if(capture_program) glDeleteProgram(capture_program);capture_program=0;
    models.clear();
    for(auto& [_,c]:canvases) destroy(c); canvases.clear(); destroy(mask);
    for(auto& [id,_]:textures) glDeleteTextures(1,&id); textures.clear();
    glDeleteBuffers(1,&vbo); glDeleteBuffers(1,&ebo); glDeleteVertexArrays(1,&vao); glDeleteProgram(program);
    initialized=false;
}
unsigned l2d_canvas_create(int w,int h) { TRY auto c=make_canvas(w,h); canvases[c.fbo]=c; return c.fbo; FAIL_RETURN(0) }
void l2d_canvas_destroy(unsigned id) { auto it=canvases.find(id); if(it!=canvases.end()) { destroy(it->second); canvases.erase(it); } }
unsigned l2d_canvas_texture(unsigned id) { TRY return canvas(id).texture; FAIL_RETURN(0) }
int l2d_canvas_clear(unsigned id,float r,float g,float b,float a) { TRY bind(canvas(id)); glClearColor(r*a,g*a,b*a,a); glClear(GL_COLOR_BUFFER_BIT); return 1; FAIL_RETURN(0) }
int l2d_read_rgba(unsigned id,unsigned char* out,size_t size) { TRY
    auto& c=canvas(id); if(!out || size!=size_t(c.width)*c.height*4) throw std::runtime_error("Readback buffer size mismatch");
    bind(c); glPixelStorei(GL_PACK_ALIGNMENT,1);
    std::vector<unsigned char> raw(size); glReadPixels(0,0,c.width,c.height,GL_RGBA,GL_UNSIGNED_BYTE,raw.data());
    for(int y=0;y<c.height;++y) for(int x=0;x<c.width;++x) {
        auto source=(size_t(c.height-1-y)*c.width+x)*4, dest=(size_t(y)*c.width+x)*4;
        unsigned a=raw[source+3]; out[dest+3]=a;
        for(int k=0;k<3;++k) out[dest+k]=a?std::min(255u,(unsigned(raw[source+k])*255+a/2)/a):0;
    }
    return 1;
FAIL_RETURN(0) }
unsigned l2d_texture_rgba(unsigned texture,int w,int h,const unsigned char* data) { TRY
    if(!data || w<1 || h<1 || w>16384 || h>16384) throw std::runtime_error("Invalid texture size");
    if(texture && !textures.count(texture)) throw std::runtime_error("Unknown texture");
    if(!texture) glGenTextures(1,&texture);
    glActiveTexture(GL_TEXTURE0); glBindTexture(GL_TEXTURE_2D,texture); glPixelStorei(GL_UNPACK_ALIGNMENT,1);
    if(textures.count(texture) && textures[texture]==std::array<int,2>{w,h}) glTexSubImage2D(GL_TEXTURE_2D,0,0,0,w,h,GL_RGBA,GL_UNSIGNED_BYTE,data);
    else glTexImage2D(GL_TEXTURE_2D,0,GL_RGBA8,w,h,0,GL_RGBA,GL_UNSIGNED_BYTE,data);
    glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_MIN_FILTER,GL_LINEAR); glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_MAG_FILTER,GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_WRAP_S,GL_CLAMP_TO_EDGE); glTexParameteri(GL_TEXTURE_2D,GL_TEXTURE_WRAP_T,GL_CLAMP_TO_EDGE);
    textures[texture]={w,h}; return texture;
FAIL_RETURN(0) }
unsigned l2d_texture_load(const char* path) { TRY
    auto file=read_file(fs::u8path(path)); int w,h,channels;
    auto* data=stbi_load_from_memory(file.data(),int(file.size()),&w,&h,&channels,4);
    if(!data) throw std::runtime_error("Image decode failed");
    auto result=l2d_texture_rgba(0,w,h,data); stbi_image_free(data); return result;
FAIL_RETURN(0) }
void l2d_texture_destroy(unsigned id) { if(textures.erase(id)) glDeleteTextures(1,&id); }
int l2d_texture_size(unsigned id,int* dimensions) { TRY
    if(!dimensions || !textures.count(id)) throw std::runtime_error("Unknown texture");
    dimensions[0]=textures[id][0];dimensions[1]=textures[id][1];return 1;
FAIL_RETURN(0) }
int l2d_draw_texture(unsigned texture,unsigned fbo,float x,float y,float w,float h,float rotation) { TRY
    if(!textures.count(texture)) throw std::runtime_error("Unknown texture");
    auto& c=canvas(fbo); bind(c);
    float cosine=std::cos(rotation),sine=std::sin(rotation);
    std::vector<float> points;
    for(auto p:std::array<std::array<float,4>,4>{{{-w/2,-h/2,0,1},{w/2,-h/2,1,1},{w/2,h/2,1,0},{-w/2,h/2,0,0}}}) {
        float aspect=float(c.width)/c.height, px=p[0]*aspect;
        points.insert(points.end(),{x+(px*cosine-p[1]*sine)/aspect,y+px*sine+p[1]*cosine,p[2],p[3]});
    }
    const unsigned short indices[]={0,1,2,0,2,3}; prepare_mesh(identity);
    mesh(texture,points,indices,6,1,white,black,0,0,false); return 1;
FAIL_RETURN(0) }
int l2d_diagnostic(unsigned id,float t) { TRY
    auto& c=canvas(id); bind(c); glEnable(GL_SCISSOR_TEST);
    int x=int(c.width*(0.35+0.1*std::sin(t))), y=int(c.height*0.25);
    glScissor(x,y,c.width/3,c.height/2); glClearColor(0.12f,0.45f,0.38f,0.7f); glClear(GL_COLOR_BUFFER_BIT); glDisable(GL_SCISSOR_TEST); return 1;
FAIL_RETURN(0) }

#include "purism_model.inc"
