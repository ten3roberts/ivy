SHADERC=glslc

SHADERS=\
				default.vert.spv\
				default.frag.spv\
				uv.frag.spv

all: shaders

shaders: $(SHADERS) 

# Compile shaders into SPIR-V
%.spv: ./res/shaders/%
	$(SHADERC) $^ -o ./res/shaders/$@

clean:
	rm ./res/shaders/*.spv
